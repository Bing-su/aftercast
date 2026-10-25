namespace Advanced_Combat_Tracker;

public partial class FormActMain
{
    private const int ActionBacklogWarning = 4096;
    private readonly Queue<(MasterSwing Action, bool Import)> actionQueue = new();
    private readonly ManualResetEventSlim actionsIdle = new(true, 0);
    private readonly ManualResetEventSlim combatTransitionsIdle = new(true, 0);
    private AutoResetEvent? actionSignal;
    private Thread? actionWorker;
    private bool actionBusy, actionsClosed, actionsStopped, backlogReported;
    private bool startingCombat, endingCombat;
    private bool actionAdmissionPaused;
    private int actionPauseOwner;
    private int combatTransitionOwner, shutdownOwner;
    private int actionProducers;
    private bool importingLogs;
    [ThreadStatic] private static int actionCallbackDepth;

    public event CombatActionDelegate? BeforeCombatAction;
    public event CombatActionDelegate? AfterCombatAction;
    public event CombatToggleEventDelegate? OnCombatStart;
    public event CombatToggleEventDelegate? OnCombatEnd;

    internal bool ImportingLogs
    {
        get { lock (CombatGate) return importingLogs; }
        set { lock (CombatGate) importingLogs = value; }
    }

    public bool SetEncounter(DateTime time, string attacker, string victim)
    {
        CombatToggleEventArgs? info;
        bool import;
        lock (CombatGate)
        {
            EnsureCombatOpen();
            EnsureCombatTransitionIdle();
            info = BeginEncounterState(time, CurrentZone);
            if (info is null) return true;
            startingCombat = true;
            combatTransitionOwner = Environment.CurrentManagedThreadId;
            combatTransitionsIdle.Reset();
            import = importingLogs;
        }
        try { NotifyCombatToggle(OnCombatStart, import, info); }
        finally
        {
            // ACT start subscribers observe InCombat=false; commit only after their synchronous callbacks.
            lock (CombatGate)
            {
                MarkEncounterStartedState(time);
                startingCombat = false;
                combatTransitionOwner = 0;
                combatTransitionsIdle.Set();
            }
        }
        return true;
    }

    public void ChangeZone(string zoneName)
    {
        lock (CombatGate)
        {
            EnsureCombatOpen();
            EnsureCombatTransitionIdle();
            ChangeZoneState(zoneName, LastKnownTime);
            CurrentZone = zoneName;
        }
    }

    public void AddCombatAction(int swingType, bool critical, string special, string attacker, string attackType,
        Dnum damage, DateTime time, int timeSorter, string victim, string damageType) =>
        AddCombatAction(new(swingType, critical, special, damage, time, timeSorter, attackType, attacker, damageType, victim));

    public void AddCombatAction(MasterSwing action)
    {
        lock (CombatGate)
        {
            EnsureCombatOpen();
            if (!InCombat) throw new InvalidOperationException("Do not add combat actions while ActGlobals.oFormActMain.InCombat is false");
            actionProducers++;
            actionsIdle.Reset();
        }
        try
        {
            action.NormalizeForHost();
            var info = new CombatActionEventArgs(action);
            lock (CombatGate) { RenameCombatant(info); RedirectAbility(info); }
            bool import;
            lock (CombatGate) import = importingLogs;
            NotifyCombatActions(BeforeCombatAction, import, info, "BeforeCombatAction subscriber", cancelable: true);
            if (info.cancelAction) return;
            if (!import)
                ActGlobals.oFormSpellTimers?.NotifySpell(info.attacker.ToLower(), info.theAttackType,
                    info.attacker == ActGlobals.charName || info.victim == ActGlobals.charName, info.victim.ToLower(), info.damage.Number >= 0,
                    new() { ["attacker"] = info.attacker, ["critical"] = info.critical.ToString(), ["damage"] = info.damage.Number.ToString(),
                        ["special"] = info.special, ["attacktype"] = info.theAttackType, ["damagetype"] = info.theDamageType,
                        ["time"] = info.time.ToLongTimeString() });
            var warn = false;
            lock (CombatGate)
            {
                // End/shutdown blocks new admissions and waits for this producer, so its accepted work can still commit.
                StartActionWorker();
                action.Apply(info);
                actionQueue.Enqueue((action, import));
                if (!backlogReported && actionQueue.Count >= ActionBacklogWarning)
                    warn = backlogReported = true;
                actionSignal!.Set();
            }
            if (warn) WriteHostWarning("The combat action queue has at least 4096 pending actions; parsing continues without dropping accepted actions", "Action queue backlog warning");
        }
        finally
        {
            lock (CombatGate) { actionProducers--; SetActionsIdleIfComplete(); }
        }
    }

    private void StartActionWorker()
    {
        if (actionWorker is not null) return;
        var signal = new AutoResetEvent(false);
        var worker = new Thread(ProcessActions) { IsBackground = true, Name = "Aftercast combat actions" };
        actionSignal = signal;
        try { worker.Start(); actionWorker = worker; }
        catch { actionSignal = null; signal.Dispose(); throw; }
    }

    private void ProcessActions()
    {
        while (true)
        {
            (MasterSwing Action, bool Import)? next = null;
            lock (CombatGate)
            {
                if (actionQueue.TryDequeue(out var item)) { next = item; actionBusy = true; }
                else if (actionsStopped) return;
            }
            if (next is not { } entry) { actionSignal!.WaitOne(); continue; }
            try
            {
                lock (CombatGate) ActiveZone.AddCombatAction(entry.Action);
                // Do not hold the graph monitor while arbitrary subscribers synchronously invoke the owner thread.
                NotifyCombatActions(AfterCombatAction, entry.Import, new(entry.Action), "AfterCombatAction subscriber");
            }
            catch (Exception error) { RecordImportFailure(error); WriteExceptionLog(error, "Combat action aggregation"); }
            finally
            {
                lock (CombatGate)
                {
                    actionBusy = false;
                    if (actionQueue.Count < ActionBacklogWarning / 2) backlogReported = false;
                    SetActionsIdleIfComplete();
                }
            }
        }
    }

    private void SetActionsIdleIfComplete()
    {
        if (actionQueue.Count == 0 && !actionBusy && actionProducers == 0) actionsIdle.Set();
    }

    private void NotifyCombatActions(CombatActionDelegate? handlers, bool import, CombatActionEventArgs info, string context, bool cancelable = false)
    {
        actionCallbackDepth++;
        try
        {
            foreach (var callback in Delegate.EnumerateInvocationList(handlers))
            {
                try { callback(import, info); }
                catch (Exception error) { RecordImportFailure(error); WriteExceptionLog(error, context); }
                // Use ACT's isolated-handler mode: a cancellation cannot be undone by later subscribers.
                if (cancelable && info.cancelAction) break;
            }
        }
        finally { actionCallbackDepth--; }
    }

    private void NotifyCombatToggle(CombatToggleEventDelegate? handlers, bool import, CombatToggleEventArgs info)
    {
        foreach (var callback in Delegate.EnumerateInvocationList(handlers))
            try { callback(import, info); }
            catch (Exception error) { RecordImportFailure(error); WriteExceptionLog(error, "Combat transition subscriber"); }
    }

    public void EndCombat(bool force)
    {
        EnsureCanWaitForCombatActions();
        bool import;
        lock (CombatGate)
        {
            EnsureImportAdmission();
            if (!InCombat) return;
            ObjectDisposedException.ThrowIf(actionsStopped, this);
            if (actionAdmissionPaused && actionPauseOwner != Environment.CurrentManagedThreadId)
                throw new InvalidOperationException("Combat transitions are suspended during plugin teardown");
            if (actionsClosed && shutdownOwner != Environment.CurrentManagedThreadId)
                throw new InvalidOperationException("Combat processing is shutting down");
            EnsureCombatTransitionIdle();
            endingCombat = true;
            combatTransitionOwner = Environment.CurrentManagedThreadId;
            combatTransitionsIdle.Reset();
            InCombat = false;
            import = importingLogs;
        }
        try
        {
            WaitForCombatActions();
            CombatToggleEventArgs info;
            lock (CombatGate) info = CompleteEncounterState();
            NotifyCombatToggle(OnCombatEnd, import, info);
            PublishCombatSnapshotState(import, info.encounter!);
        }
        finally
        {
            lock (CombatGate)
            {
                endingCombat = false;
                combatTransitionOwner = 0;
                combatTransitionsIdle.Set();
            }
        }
        // The original force flag controls UI/export paths; an embedded library performs neither automatically.
    }

    internal void CheckIdleEndCombat()
    {
        bool end;
        lock (CombatGate) end = InCombat && LastKnownTime - LastHostileTime > TimeSpan.FromSeconds(6);
        if (end) EndCombat(true);
    }

    internal void EnsureCanWaitForCombatActions()
    {
        lock (CombatGate)
        {
            if (actionCallbackDepth > 0 || Thread.CurrentThread == actionWorker ||
                Environment.CurrentManagedThreadId == ownerThread && IsDispatchingCallback?.Invoke() == true &&
                (actionBusy || actionProducers > 0))
                throw new InvalidOperationException("Cannot wait for combat actions from their own callback");
        }
    }

    internal void EnsureCanWaitForCombatLifecycle()
    {
        EnsureCanWaitForCombatActions();
        lock (CombatGate)
        {
            if (activeImport is not null) throw new InvalidOperationException("Log import is already in progress");
            if (actionAdmissionPaused) throw new InvalidOperationException("Plugin teardown is already in progress");
            if ((startingCombat || endingCombat) && (combatTransitionOwner == Environment.CurrentManagedThreadId ||
                Environment.CurrentManagedThreadId == ownerThread && IsDispatchingCallback?.Invoke() == true))
                throw new InvalidOperationException("Cannot wait for a combat transition from its own callback");
        }
    }

    internal void RunWithCombatActionsSuspended(Action action)
    {
        lock (CombatGate)
        {
            EnsureCanWaitForCombatLifecycle();
            actionAdmissionPaused = true;
            actionPauseOwner = Environment.CurrentManagedThreadId;
        }
        try { WaitForCombatTransitions(); WaitForCombatActions(); action(); }
        finally { lock (CombatGate) { actionAdmissionPaused = false; actionPauseOwner = 0; } }
    }

    internal void WaitForCombatActions()
    {
        EnsureCanWaitForCombatActions();
        lock (CombatGate) { if (actionsStopped) return; }
        while (!actionsIdle.Wait(10)) PumpOwnerCallbacks();
    }

    internal void StopCombatProcessing()
    {
        BeginCombatShutdown();
        lock (CombatGate) { if (actionsStopped) return; }
        var errors = new List<Exception>();
        try { WaitForCombatTransitions(); } catch (Exception error) { errors.Add(error); }
        try { WaitForCombatActions(); } catch (Exception error) { errors.Add(error); }
        try { EndCombat(true); } catch (Exception error) { errors.Add(error); }
        lock (CombatGate) { actionsStopped = true; actionSignal?.Set(); }
        if (actionWorker is { } worker)
            while (!worker.Join(10)) PumpOwnerCallbacks();
        actionSignal?.Dispose();
        actionsIdle.Dispose();
        combatTransitionsIdle.Dispose();
        if (errors.Count != 0) throw new AggregateException("Combat processing shutdown failed", errors);
    }

    internal void BeginCombatShutdown()
    {
        lock (CombatGate)
        {
            EnsureCanWaitForCombatLifecycle();
            if (actionsStopped) return;
            if (actionsClosed && shutdownOwner != Environment.CurrentManagedThreadId)
                throw new InvalidOperationException("Combat processing is already shutting down");
            // Close admission before reader joins; an already-running transition is counted and drained below.
            actionsClosed = true;
            shutdownOwner = Environment.CurrentManagedThreadId;
        }
    }

    private void WaitForCombatTransitions()
    {
        lock (CombatGate) { if (actionsStopped) return; }
        while (!combatTransitionsIdle.Wait(10)) PumpOwnerCallbacks();
    }

    private void EnsureCombatOpen()
    {
        ObjectDisposedException.ThrowIf(actionsClosed, this);
        EnsureImportAdmission();
        if (actionAdmissionPaused) throw new InvalidOperationException("Combat action admission is suspended during plugin teardown");
    }
    private void EnsureCombatTransitionIdle()
    {
        if (startingCombat || endingCombat) throw new InvalidOperationException("A combat transition is already in progress");
    }

    public void ActCommands(string command)
    {
        // ACT compares this literal before any normalization; headless completion omits UI refresh and command sound.
        if (command == "end")
        {
            if (InCombat) EndCombat(true);
            return;
        }
        if (command == "reset players") { SelectiveListUncheckAll(); return; }
        if (command is not null && command.StartsWith("add ", StringComparison.Ordinal)) { SelectiveListAdd(command[4..]); return; }
        if (command is not null && command.StartsWith("remove ", StringComparison.Ordinal)) { SelectiveListRemove(command[7..], false); return; }
        if (command is null or "macro all" or "macro last" or "clip all" or "clip last" or "clear html" or "clear"
            || command.StartsWith("macro: ", StringComparison.Ordinal)
            || command.StartsWith("clip last-", StringComparison.Ordinal)
            || command.StartsWith("timer ", StringComparison.Ordinal))
            throw new NotSupportedException("This ACT UI/chat command is not implemented by the embedded host");
        // ACT ignores unmatched echo text, e.g. "Hojoring>[EX] Beacon ..."; it is not a failed command.
    }
}
