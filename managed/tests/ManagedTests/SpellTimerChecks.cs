using Advanced_Combat_Tracker;
using Aftercast.ActCompat;
using Aftercast.Testing;

internal static class SpellTimerChecks
{
    internal static void Run()
    {
        var directory = Directory.CreateTempSubdirectory("aftercast-spell-timers-");
        try
        {
            using var host = new HostEnvironment(directory.FullName);
            CombatFixture.ConfigureCategories();
            var root = ActGlobals.oFormActMain!;
            var timers = ActGlobals.oFormSpellTimers!;
            var epoch = DateTime.UnixEpoch;
            root.LastKnownTime = epoch;
            root.LastKnownTime = DateTime.MinValue;
            if (root.LastKnownTime != epoch) throw new Exception("ACT ignored-time sentinel changed");
            var data = new TimerData("Hit", "Fixture") { OnlyMasterTicks = true, TimerValue = 3, WarningValue = 1 };
            timers.AddEditTimerDef(data);
            timers.RebuildSpellTreeView();
            var notifications = 0;
            timers.OnSpellTimerNotify += frame =>
            {
                if (frame.Name != "Hit") return;
                notifications++;
                if (frame.Combatant != "source" || frame.SpellTimers[^1].ExtraInfo["damage"] != "88")
                    throw new Exception("Timer did not observe edited combat data");
            };
            root.BeforeCombatAction += (_, info) => { info.damage = new Dnum(88); info.cancelAction = info.timeSorter == 2; };
            root.SetEncounter(epoch, "Source", "Target");
            root.AddCombatAction(1, false, "None", "Source", "Hit", new Dnum(100), epoch, 1, "Target", "Physical");
            root.AddCombatAction(1, false, "None", "Source", "Hit", new Dnum(100), epoch, 2, "Target", "Physical");
            root.EndCombat(true);
            if (notifications != 1 || timers.GetTimerFrames().Count != 1) throw new Exception("Cancelled action notified a timer");
            var events = new List<string>();
            timers.OnSpellTimerWarning += _ => events.Add("warning");
            timers.OnSpellTimerExpire += _ => events.Add("expire");
            timers.OnSpellTimerRemoved += _ => events.Add("removed");
            root.LastKnownTime = epoch.AddSeconds(4);
            host.AdvanceTimers();
            host.AdvanceTimers();
            if (!events.SequenceEqual(new[] { "warning", "expire" })) throw new Exception("Timer warning/expiry order changed");
            timers.GetTimerFrames()[0].SpellTimers.Clear();
            host.AdvanceTimers();
            if (timers.GetTimerFrames().Count != 0 || events[^1] != "removed") throw new Exception("Timer cancellation was not retired");
            var path = Path.Combine(directory.FullName, "import.log");
            File.WriteAllText(path, "fixture\n");
            root.GetDateTimeFromLog = _ => epoch;
            root.OnLogLineRead += (_, _) =>
            {
                Thread.Sleep(20);
                root.SetEncounter(epoch, "Source", "Target");
                root.AddCombatAction(1, false, "None", "Source", "Hit", new Dnum(100), epoch, 3, "Target", "Physical");
            };
            Thread.Sleep(40);
            var estimate = root.LastEstimatedTime;
            host.ImportLog(path);
            if (notifications != 1 || timers.GetTimerFrames().Count != 0 || root.LastEstimatedTime < estimate)
                throw new Exception("Import fired a live timer or reset the live clock estimate");
            var ticks = new TimerData("Tick", "Fixture") { TimerValue = 30 };
            timers.AddEditTimerDef(ticks);
            timers.RebuildSpellTreeView();
            root.LastKnownTime = epoch.AddSeconds(20);
            timers.NotifySpell("Caster", "Tick", true, "", false);
            root.LastKnownTime = epoch.AddSeconds(21);
            timers.NotifySpell("Caster", "Tick", true, "", true);
            root.LastKnownTime = epoch.AddSeconds(23);
            timers.NotifySpell("Caster", "Tick", true, "", true);
            root.LastKnownTime = epoch.AddSeconds(36);
            timers.NotifySpell("Caster", "Tick", true, "", true);
            var frame = timers.GetTimerFrames().Single();
            if (frame.SpellTimers.Count != 3 || !frame.SpellTimers[0].MasterTimer || frame.SpellTimers[1].MasterTimer || !frame.SpellTimers[2].MasterTimer)
                throw new Exception("Duplicate/nonmaster/master notification windows changed");
            // A stepped import may pump live timers; historical/future parser timestamps must not move their clock.
            root.GetDateTimeFromLog = _ => epoch.AddYears(30);
            var liveEstimate = root.LastEstimatedTime;
            var expired = events.Count;
            host.BeginImport(path);
            while (host.AdvanceImport(false) is null)
            {
                host.AdvanceTimers();
                if (root.LastEstimatedTime < liveEstimate || root.LastEstimatedTime > liveEstimate.AddSeconds(5))
                    throw new Exception("Import timestamps replaced the live timer clock");
                Thread.Yield();
            }
            if (events.Count != expired) throw new Exception("Future-dated import expired a live timer");
            if (host.Diagnostics.Item1 != 0) throw new Exception("Spell timer contract produced diagnostics");
        }
        finally { directory.Delete(true); }
        if (ActGlobals.oFormSpellTimers is not null) throw new Exception("Timer globals survived host disposal");
        Console.WriteLine("Action-driven timer notifications, cancellation, warning/expiry order and import clock restoration passed.");
    }
}
