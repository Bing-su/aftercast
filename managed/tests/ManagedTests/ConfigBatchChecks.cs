using System.Security;
using Advanced_Combat_Tracker;
using Aftercast.ActCompat;

internal static class ConfigBatchChecks
{
    private static void Check(bool value, string message) { if (!value) throw new InvalidOperationException(message); }
    private static void Reject(Action action)
    {
        try { action(); }
        catch (Exception error) when (error is ArgumentException or InvalidOperationException or IOException or InvalidDataException or System.Xml.XmlException) { return; }
        throw new Exception("Expected rejected Config");
    }
    internal static void Run()
    {
        var directory = Directory.CreateTempSubdirectory("aftercast-config-batch-");
        try
        {
            using (var host = new HostEnvironment(directory.FullName))
            {
                host.ConfigureSpeech(true);
                var main = ActGlobals.oFormActMain!;
                var timers = ActGlobals.oFormSpellTimers!;
                var mixed=host.PreviewMixedConfig("<Config><CustomTriggers><Trigger Category='Fixture' Regex='mixed'/></CustomTriggers><SpellTimers><Spell Name='Mixed' Category='Fixture'/></SpellTimers></Config>");
                Check(mixed.Triggers.Items.Length==1 && mixed.Timers.Items.Length==1 && main.ReadTriggers(0,16).Total==0 && timers.ReadTimerDefinitions(0,16).Total==0,"Mixed preview must not publish");
                Reject(()=>host.PreviewMixedConfig("<Config><CustomTriggers><Trigger Category='x' Regex='['/></CustomTriggers><SpellTimers><Spell Name='Mixed'/></SpellTimers></Config>"));
                Reject(()=>host.PreviewMixedConfig("<Config><CustomTriggers><Trigger Category='x' Regex='ok'/></CustomTriggers><SpellTimers><Spell Name='Mixed' Timer='bad'/></SpellTimers></Config>"));
                Reject(()=>host.PreviewMixedConfig("<Config><CustomTriggers>"+string.Concat(Enumerable.Repeat("<Trigger Category='x' Regex='a'/>",32))+"</CustomTriggers><SpellTimers><Spell Name='one'/></SpellTimers></Config>"));
                const string trigger = "<Config><CustomTriggers><Trigger Category='Fixture' Regex='one' Active='True' SoundType='3' SoundData='no sound' Timer='True' Tabbed='True'/><Trigger Category='Fixture' Regex='two'/></CustomTriggers></Config>";
                var preview = main.PreviewTriggerConfig(trigger);
                Check(preview.Items.Length == 2 && preview.Conflicts.Length == 0 && main.ReadTriggers(0,16).Total == 0, "Preview changed trigger registry");
                var wrapped = "<Config Xml=\"" + SecurityElement.Escape(trigger) + "\"/>";
                Check(main.PreviewTriggerConfig(wrapped).Items.Length == 2, "ACT Config wrapper not decoded");
                var forbidden = "<Config><CustomTriggers><Trigger Regex='logFilePath' Category='Fixture'/></CustomTriggers></Config>";
                var forbiddenWrapper = "<Config Xml=\"" + SecurityElement.Escape(forbidden) + "\"/>";
                Reject(() => main.PreviewTriggerConfig(forbiddenWrapper));
                Check(main.PreviewTriggerConfig(forbidden).Items.Length == 1, "Direct allowlisted Config should not be treated as a shared wrapper");
                foreach (var bad in new[] { "<Config><CustomTriggers><Trigger Regex='x' Category='y'/><Trigger Regex='[' Category='y'/></CustomTriggers></Config>", "<Config><CustomTriggers><Trigger Regex='x' Category='y'/><Trigger Regex='x' Category='y'/></CustomTriggers></Config>", "<Config><CustomTriggers/><SpellTimers/></Config>", "<Config><SettingsSerializer/></Config>", "<!DOCTYPE Config [<!ENTITY x SYSTEM 'file:///etc/passwd'>]><Config><CustomTriggers/></Config>", "<Config><CustomTriggers>" + string.Concat(Enumerable.Repeat("<Trigger Regex='x' Category='y'/>",33)) + "</CustomTriggers></Config>", new string('x',16385) }) Reject(() => main.PreviewTriggerConfig(bad));
                var values = preview.Items.Select(item => item.Definition).ToArray();
                var triggerPath = Path.Combine(directory.FullName,"CustomTriggers.xml");
                Directory.CreateDirectory(triggerPath);
                Reject(() => main.SaveTriggerBatch(preview.Revision,values));
                Check(main.ReadTriggers(0,16).Total == 0, "Failed batch save published triggers");
                Directory.Delete(triggerPath);
                var changed = main.SaveTrigger(preview.Revision, null, values[0]);
                Reject(() => main.SaveTriggerBatch(preview.Revision,values));
                var conflicting = main.PreviewTriggerConfig(trigger);
                Check(conflicting.Conflicts.Length == 1, "Existing trigger not reported");
                var persisted = File.ReadAllBytes(triggerPath);
                Reject(() => main.SaveTriggerBatch(changed,values));
                Check(main.ReadTriggers(0,16).Total == 1 && File.ReadAllBytes(triggerPath).SequenceEqual(persisted), "Conflicting batch changed trigger state");
                var second = main.PreviewTriggerConfig("<Config><CustomTriggers><Trigger Category='Fixture' Regex='two'/><Trigger Category='Fixture' Regex='three'/></CustomTriggers></Config>");
                main.SaveTriggerBatch(second.Revision, second.Items.Select(item => item.Definition).ToArray());
                Check(main.ReadTriggers(0,16).Total == 3, "Trigger batch did not publish all");
                const string timer = "<Config><SpellTimers><Spell Name='A' Category='Fixture' Timer='45' Checked='True' StartWav='speak test'/><Spell Name='B' Category='Fixture' WarningValue='5' /></SpellTimers></Config>";
                var timerPreview = timers.PreviewTimerConfig(timer);
                Check(timerPreview.Items.Length == 2 && timerPreview.Conflicts.Length == 0 && timers.ReadTimerDefinitions(0,16).Total == 0, "Preview changed timer registry");
                Reject(() => timers.PreviewTimerConfig("<Config><SpellTimers><Spell Name='a' Category='fixture'/><Spell Name='A' Category='Fixture'/></SpellTimers></Config>"));
                Reject(() => timers.PreviewTimerConfig("<Config><SpellTimers><Spell Name='a' Category='x' Timer='bad'/></SpellTimers></Config>"));
                Reject(() => timers.PreviewTimerConfig("<Config><SpellTimers><Spell Name='a' Category='x' Timer='3601'/></SpellTimers></Config>"));
                var timerPath = Path.Combine(directory.FullName,"SpellTimers.xml");
                Directory.CreateDirectory(timerPath);
                var timerValues = timerPreview.Items.Select(item => item.Definition).ToArray();
                Reject(() => timers.SaveTimerBatch(timerPath,timerPreview.Revision,timerValues));
                Check(timers.ReadTimerDefinitions(0,16).Total == 0, "Failed batch save published timers");
                Directory.Delete(timerPath);
                var triggerBytes=File.ReadAllBytes(triggerPath);
                using(var lease=new FileStream(Path.Combine(directory.FullName,".mixed-config.lock"),FileMode.OpenOrCreate,FileAccess.ReadWrite,FileShare.None))
                {
                    var current=main.ReadTriggers(0,16);
                    Reject(()=>main.SaveTriggerOptions(current.Revision,!current.OriginalLines,current.ProcessImports));
                    Reject(()=>timers.SaveTimerBatch(timerPath,timerPreview.Revision,timerValues));
                    Check(main.ReadTriggers(0,16).Total==3 && timers.ReadTimerDefinitions(0,16).Total==0 && File.ReadAllBytes(triggerPath).SequenceEqual(triggerBytes) && !File.Exists(timerPath),"single saves must respect mixed lease without publishing");
                }
                timers.SaveTimerBatch(timerPath,timerPreview.Revision,timerValues);
                Reject(() => timers.SaveTimerBatch(timerPath,timerPreview.Revision,timerValues));
                Check(timers.ReadTimerDefinitions(0,16).Total == 2 && timers.GetTimerFrames().Count == 0 && host.DrainSpeech().Length == 0, "Timer batch published incorrectly or ran effects");
            }
            using (var restarted = new HostEnvironment(directory.FullName))
                Check(ActGlobals.oFormActMain!.ReadTriggers(0,16).Total == 3 && ActGlobals.oFormSpellTimers!.ReadTimerDefinitions(0,16).Total == 2, "Batch restart mismatch");
            Console.WriteLine("Config homogeneous batch allowlist, stale/conflict, atomic save, effects and restart passed.");
        }
        finally { directory.Delete(true); }
    }
}
