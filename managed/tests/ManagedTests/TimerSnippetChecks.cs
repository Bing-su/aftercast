using Advanced_Combat_Tracker;
using Aftercast.ActCompat;

internal static class TimerSnippetChecks
{
    private static void Check(bool value, string message) { if (!value) throw new InvalidOperationException(message); }
    private static void Reject(Action action)
    {
        try { action(); }
        catch (Exception error) when (error is ArgumentException or InvalidOperationException or IOException or InvalidDataException or System.Xml.XmlException) { return; }
        throw new Exception("Expected rejected Spell snippet");
    }
    internal static void Run()
    {
        var directory = Directory.CreateTempSubdirectory("aftercast-spell-snippet-");
        try
        {
            using (var host = new HostEnvironment(directory.FullName))
            {
                var timers = ActGlobals.oFormSpellTimers!;
                host.ConfigureSpeech(true);
                var defaults = timers.PreviewTimerSnippet("<Spell N='Foo' C='' />");
                Check(defaults.Definition.Category == " General" && defaults.Definition.TimerValue == 30 && defaults.Definition.WarningValue == 10 &&
                    defaults.Definition.RemoveValue == -15 && defaults.Definition.RadialDisplay && defaults.Definition.Modable && defaults.Definition.Panel1Display,
                    "ACT new-timer defaults differ");
                const string xml = "<Spell N='FoO' C='Fixture' T='41' R='T' OM='T' A='T' WV='3' RD='F' M='F' Tt='&lt;note&gt;' SS='speak &amp; test' WS='warn' FC='-1' RV='-7' RC='T' />";
                var preview = timers.PreviewTimerSnippet(xml);
                var item = preview.Definition;
                Check(!preview.Exists && item.Name == "FoO" && item.Category == "Fixture" && item.TimerValue == 41 && item.RestrictToMe && item.OnlyMasterTicks && item.AbsoluteTiming &&
                    item.WarningValue == 3 && !item.RadialDisplay && !item.Modable && item.Tooltip == "<note>" && item.StartSoundData == "speak & test" && item.WarningSoundData == "warn" &&
                    item.FillColorArgb == -1 && item.RemoveValue == -7 && item.RestrictToCategory, "Spell attributes not decoded");
                Check(timers.ReadTimerDefinitions(0,16).Total == 0 && timers.GetTimerFrames().Count == 0 && host.DrainSpeech().Length == 0, "Preview mutated timers or played audio");
                var malformedNumber = timers.PreviewTimerSnippet("<Spell N='foo' C='' T='bad' WV='bad' RV='bad' FC='bad' />").Definition;
                Check(malformedNumber.TimerValue == 30 && malformedNumber.WarningValue == 10 && malformedNumber.RemoveValue == -15 && malformedNumber.FillColorArgb == -16776961, "ACT malformed numeric fallback differs");
                foreach (var bad in new[] { "<Trigger R='x' C='y'/>", "<Spell N='x'/>", "<Spell N='' C='y'/>", "<Spell N='x' C='y' X='1'/>", "<Spell N='x' C='y' T='3601'/>", "<Spell N='x' C='y'><Spell /></Spell>", "<!DOCTYPE Spell [<!ENTITY x SYSTEM 'file:///etc/passwd'>]><Spell N='&x;' C='y'/>", new string('x', 16385) }) Reject(() => timers.PreviewTimerSnippet(bad));
                var blocked = Path.Combine(directory.FullName, "SpellTimers.xml");
                Directory.CreateDirectory(blocked);
                Reject(() => host.TimerOperation(2, preview.Revision, null, item, null, 0, 16));
                Check(timers.ReadTimerDefinitions(0,16).Total == 0, "Failed save published timer");
                Directory.Delete(blocked);
                var saved = (SavedTimer)host.TimerOperation(2, preview.Revision, null, item, null, 0, 16);
                Check(timers.PreviewTimerSnippet("<Spell N='foo' C='fixture' />").Exists, "Case collision missed");
                Reject(() => host.TimerOperation(2, saved.Revision, null, item, null, 0, 16));
                Reject(() => host.TimerOperation(2, preview.Revision, null, item, null, 0, 16));
                Check(timers.GetTimerFrames().Count == 0 && host.DrainSpeech().Length == 0, "Import started timer or sound");
            }
            using (var restarted = new HostEnvironment(directory.FullName))
                Check(ActGlobals.oFormSpellTimers!.ReadTimerDefinitions(0,16).Total == 1, "Imported Spell did not survive restart");
            Console.WriteLine("Spell snippet defaults, attributes, ranges, XML safety, atomic failure, conflict and restart passed.");
        }
        finally { directory.Delete(true); }
    }
}
