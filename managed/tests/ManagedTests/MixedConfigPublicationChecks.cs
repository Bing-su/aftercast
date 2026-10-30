using System.Reflection;
using System.Runtime.ExceptionServices;
using Advanced_Combat_Tracker;
using Aftercast.ActCompat;
using Aftercast.Bridge;

internal static class MixedConfigPublicationChecks
{
    private const string Xml = "<Config><CustomTriggers><Trigger Category='Fixture' Regex='mixed'/></CustomTriggers><SpellTimers><Spell Name='Mixed' Category='Fixture'/></SpellTimers></Config>";
    private static void Check(bool value, string message) { if (!value) throw new Exception(message); }
    private static void Inject(HostEnvironment host, ulong triggers, ulong timers, Action<string> fault)
    {
        try { typeof(HostEnvironment).GetMethod("ImportMixedConfigCore", BindingFlags.NonPublic | BindingFlags.Instance)!.Invoke(host, [Xml, triggers, timers, fault]); }
        catch (TargetInvocationException error) { ExceptionDispatchInfo.Capture(error.InnerException!).Throw(); }
    }
    private static void Reject(Action action)
    {
        try { action(); } catch (Exception error) when (error is IOException or InvalidOperationException or ObjectDisposedException) { return; }
        throw new Exception("Mixed publication should fail");
    }
    public static void Run()
    {
        foreach (var stage in new[] { "success", "before-commit", "committed", "triggers-published", "timers-published", "conflict", "lease", "stale", "contention", "retry", "files:journal", "files:CustomTriggers.xml", "files:SpellTimers.xml", "files:before-commit" })
        {
            var directory = Directory.CreateTempSubdirectory("aftercast-mixed-publication-");
            try
            {
                using (var host = new HostEnvironment(directory.FullName))
                {
                    var main = ActGlobals.oFormActMain!;
                    var timers = ActGlobals.oFormSpellTimers!;
                    // Materialize a real old pair: fail-stop Dispose must never save it over the new pair.
                    var old = main.PreviewTriggerConfig("<Config><CustomTriggers><Trigger Category='Fixture' Regex='old'/></CustomTriggers></Config>");
                    main.SaveTriggerBatch(old.Revision, old.Items.Select(item => item.Definition).ToArray());
                    var oldTimers = timers.PreviewTimerConfig("<Config><SpellTimers><Spell Name='Old' Category='Fixture'/></SpellTimers></Config>");
                    timers.SaveTimerBatch(Path.Combine(directory.FullName, "SpellTimers.xml"), oldTimers.Revision, oldTimers.Items.Select(item => item.Definition).ToArray());
                    var preview = host.PreviewMixedConfig(Xml);
                    var triggerPath = Path.Combine(directory.FullName, "CustomTriggers.xml");
                    var before = File.ReadAllBytes(triggerPath);
                    Action<string> fault = step =>
                    {
                        if (step == "before-commit" && stage == "conflict") File.AppendAllText(triggerPath, " ");
                        if (step == stage || stage == "retry" && step == "before-commit") throw new IOException("Injected " + stage);
                    };
                    if (stage == "contention")
                    {
                        using var attempted = new CountdownEvent(2);
                        Task? triggerSave = null, timerSave = null;
                        Inject(host, preview.Triggers.Revision, preview.Timers.Revision, step =>
                        {
                            if (step != "before-commit") return;
                            triggerSave = Task.Run(() => { attempted.Signal(); Reject(() => main.SaveTriggerBatch(preview.Triggers.Revision, preview.Triggers.Items.Select(item => item.Definition).ToArray())); });
                            timerSave = Task.Run(() => { attempted.Signal(); Reject(() => timers.SaveTimerBatch(Path.Combine(directory.FullName, "SpellTimers.xml"), preview.Timers.Revision, preview.Timers.Items.Select(item => item.Definition).ToArray())); });
                            Check(attempted.Wait(TimeSpan.FromSeconds(5)), "Standalone writers did not start");
                        });
                        Task.WaitAll(triggerSave!, timerSave!);
                    }
                    else if (stage == "success")
                    {
                        host.ImportMixedConfig(Xml, preview.Triggers.Revision, preview.Timers.Revision);
                        Reject(() => host.ImportMixedConfig(Xml, preview.Triggers.Revision, preview.Timers.Revision));
                        Check(main.ReadTriggers(0,16).Total == 2 && timers.ReadTimerDefinitions(0,16).Total == 2, "Mixed success/retry registry mismatch");
                        Check(host.DrainSpeech().Length == 0 && timers.GetTimerFrames().Count == 0, "Mixed import ran effects");
                    }
                    else if (stage == "lease")
                    {
                        using var lease = new FileStream(Path.Combine(directory.FullName, ".mixed-config.lock"), FileMode.OpenOrCreate, FileAccess.ReadWrite, FileShare.None);
                        Reject(() => Inject(host, preview.Triggers.Revision, preview.Timers.Revision, fault));
                    }
                    else if (stage == "stale") Reject(() => host.ImportMixedConfig(Xml, 0, preview.Timers.Revision));
                    else Reject(() => Inject(host, preview.Triggers.Revision, preview.Timers.Revision, fault));
                    if (stage == "retry") host.ImportMixedConfig(Xml, preview.Triggers.Revision, preview.Timers.Revision);
                    var committed = stage is "success" or "contention" or "retry" or "committed" or "triggers-published" or "timers-published";
                    Check(host.IsFaulted == (stage == "conflict" || committed && stage is not ("success" or "contention" or "retry")), "Unexpected fail-stop state");
                    if (host.IsFaulted)
                    {
                        Check(ActGlobals.oFormActMain is null && ActGlobals.oFormSpellTimers is null, "Faulted host retained ACT globals");
                        Reject(() => host.ImportMixedConfig(Xml, preview.Triggers.Revision, preview.Timers.Revision));
                        // Existing request entry must reject a retained faulted environment, not resurrect it.
                        var session = new PluginSession();
                        typeof(PluginSession).GetField("environment", BindingFlags.NonPublic | BindingFlags.Instance)!.SetValue(session, host);
                        Reject(() => session.ReadCombatSnapshot(0));
                        session.Stop();
                        Reject(() => session.Configure(directory.FullName));
                    }
                    else if (!committed) Check(main.ReadTriggers(0,16).Total == 1 && timers.ReadTimerDefinitions(0,16).Total == 1, "Rejected import changed memory");
                    if (stage == "conflict")
                    {
                        Check(File.ReadAllBytes(triggerPath).SequenceEqual(before.Concat(new byte[] { 32 })), "External edit overwritten");
                    }
                    Check(!Directory.EnumerateDirectories(directory.FullName, ".mixed-stage-*").Any(), "Staging leaked");
                }
                using var restarted = new HostEnvironment(directory.FullName);
                var expected = stage is "success" or "contention" or "retry" or "committed" or "triggers-published" or "timers-published" ? 2 : 1;
                Check(ActGlobals.oFormActMain!.ReadTriggers(0,16).Total == expected && ActGlobals.oFormSpellTimers!.ReadTimerDefinitions(0,16).Total == expected, "Restart lost committed pair or published rejected import");
            }
            finally { directory.Delete(true); }
        }
        Console.WriteLine("Mixed Config publication, fail-stop Dispose, conflict/lease/stale, retry and restart passed.");
    }
}
