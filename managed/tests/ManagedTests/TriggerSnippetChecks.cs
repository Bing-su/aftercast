using Advanced_Combat_Tracker;
using Aftercast.ActCompat;

internal static class TriggerSnippetChecks
{
    private static void Check(bool ok, string message) { if (!ok) throw new InvalidOperationException(message); }
    private static void Reject(Action action)
    {
        try { action(); }
        catch (Exception error) when (error is ArgumentException or InvalidOperationException or IOException or InvalidDataException or System.Xml.XmlException) { return; }
        throw new Exception("Expected rejected snippet");
    }
    internal static void Run()
    {
        var directory = Directory.CreateTempSubdirectory("aftercast-snippet-");
        try
        {
            using (var host = new HostEnvironment(directory.FullName))
            {
                var root = ActGlobals.oFormActMain!;
                host.ConfigureSpeech(true);
                const string xml = "<Trigger R=\"Source&#92;s+&#35;(?&lt;value&gt;[0-9]+)\" C=\"Fixture\" SD=\"&lt;speak&gt; &amp; hi\" ST=\"3\" T=\"T\" TN=\"Timer\" Ta=\"T\" />";
                var preview = root.PreviewTriggerSnippet(xml);
                Check(preview.Definition.Regex == @"Source\s+#(?<value>[0-9]+)" && preview.Definition.SoundData == "<speak> & hi" && preview.Definition.Tabbed && !preview.Exists, "Incorrect ACT attribute decoding");
                Check(root.ReadTriggers(0,16).Total == 0 && !File.Exists(Path.Combine(directory.FullName, "CustomTriggers.xml")), "Preview mutated state");
                foreach (var bad in new[] { "<Spell N='x' C='y'/>", "<Config Xml='x'/>", "<Trigger R='x'/>", "<Trigger R='x' C='y' X='1'/>", "<Trigger R='[' C='y'/>", "<Trigger R='x' C='y'>&xxe;</Trigger>", "<!DOCTYPE Trigger [<!ENTITY xxe SYSTEM 'file:///etc/passwd'>]><Trigger R='x' C='y'/>", new string('x', 16385) })
                    Reject(() => root.PreviewTriggerSnippet(bad));
                var blocked = Path.Combine(directory.FullName, "CustomTriggers.xml");
                Directory.CreateDirectory(blocked);
                Reject(() => root.SaveTrigger(preview.Revision, null, preview.Definition));
                Check(root.ReadTriggers(0,16).Total == 0, "Failed persistence published the snippet");
                Directory.Delete(blocked);
                var next = root.SaveTrigger(preview.Revision, null, preview.Definition);
                Check(host.DrainSpeech().Length == 0 && ActGlobals.oFormSpellTimers!.GetTimerFrames().Count == 0, "Import ran trigger effects");
                Check(root.PreviewTriggerSnippet(xml).Exists, "Duplicate not detected");
                Reject(() => root.SaveTrigger(next, null, preview.Definition));
                Reject(() => root.SaveTrigger(preview.Revision, null, preview.Definition));
                Check(root.ReadTriggers(0,16).Total == 1, "Duplicate or stale commit modified definitions");
            }
            using (var restarted = new HostEnvironment(directory.FullName))
                Check(ActGlobals.oFormActMain!.ReadTriggers(0,16).Total == 1, "Imported trigger did not survive restart");
            Console.WriteLine("Trigger snippet preview, XML safety, stale and duplicate import, effect isolation and restart passed.");
        }
        finally { directory.Delete(true); }
    }
}
