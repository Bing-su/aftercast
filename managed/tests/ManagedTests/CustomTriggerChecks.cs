using System.Reflection;
using System.Text;
using System.Text.RegularExpressions;
using System.Xml;
using Advanced_Combat_Tracker;
using Aftercast.ActCompat;

internal static class CustomTriggerChecks
{
    private static void Check(bool value, [System.Runtime.CompilerServices.CallerArgumentExpression(nameof(value))] string? expression = null)
    { if (!value) throw new Exception("Custom trigger check failed: " + expression); }

    private static void Process(FormActMain root, string original, string modified, bool import = false) =>
        typeof(FormActMain).GetMethod("ProcessCustomTriggers", BindingFlags.NonPublic | BindingFlags.Instance)!
            .Invoke(root, [original, new LogLineEventArgs(modified, 0, DateTime.UnixEpoch, "Fixture", false), import]);

    internal static void Run()
    {
        var directory = Directory.CreateTempSubdirectory("aftercast-triggers-");
        // Match Rust's canonical Windows paths to cover persisted triggers across worker restarts.
        if (OperatingSystem.IsWindows()) directory = new DirectoryInfo(@"\\?\" + directory.FullName);
        var path = Path.Combine(directory.FullName, "CustomTriggers.xml");
        try
        {
            using (var host = new HostEnvironment(directory.FullName))
            {
                host.ConfigureSpeech(true);
                var root = ActGlobals.oFormActMain!;
                root.CurrentZone = "Fixture Zone 2";
                ActGlobals.charName = "Source";
                var trigger = new CustomTrigger(@"(?<YOU>\w+) (?<value>\d+)", "fixture")
                { SoundType = 3, SoundData = "${value}", RestrictToCategoryZone = true, Timer = true, TimerName = "Countdown" };
                ActGlobals.oFormSpellTimers!.AddEditTimerDef(new TimerData("Countdown", "fixture") { TimerValue = 30 });
                ActGlobals.oFormSpellTimers.RebuildSpellTreeView();
                root.AddEditCustomTrigger(trigger);
                Process(root, "Other 1", "Other 1");
                Check(host.DrainSpeech().Length == 0);
                Process(root, "Source 1", "Source 42", true);
                Check(host.DrainSpeech().Length == 0 && ActGlobals.oFormSpellTimers.GetTimerFrames().Count == 0);
                Process(root, "Source 1", "Source 42");
                Check(Encoding.UTF8.GetString(host.DrainSpeech()) == "42" && ActGlobals.oFormSpellTimers.GetTimerFrames().Count == 1);
                Process(root, "Source 2", "Source 43");
                Check(host.DrainSpeech().Length == 0);
                trigger.LastAudioAlert = DateTime.MinValue;
                root.OriginalLoglineCustomTriggers = true;
                Process(root, "Source 7", "Source 44");
                Check(Encoding.UTF8.GetString(host.DrainSpeech()) == "7");
                trigger.LastAudioAlert = DateTime.MinValue;
                root.CurrentZone = "Elsewhere";
                Process(root, "Source 9", "Source 9");
                Check(host.DrainSpeech().Length == 0);
                root.CurrentZone = "Fixture";
                trigger.Active = false;
                Process(root, "Source 9", "Source 9");
                Check(host.DrainSpeech().Length == 0);
                trigger.Active = true;
                var bad = new CustomTrigger("(a+)+$", "Bad") { SoundType = 3, SoundData = "bad" };
                root.AddEditCustomTrigger(bad);
                Process(root, new string('a', 10000) + "!", "");
                Check(host.DiagnosticsSnapshot.Item1 == 1);
                root.CustomTriggers.Remove(bad.Key);

                // Exercise the actual follower route, including parser edits before trigger selection.
                var log = Path.Combine(directory.FullName, "live.log");
                File.WriteAllText(log, "");
                root.OriginalLoglineCustomTriggers = false;
                root.BeforeLogLineRead += (_, info) => info.logLine = "Source 88";
                root.LogFilePath = log;
                root.OpenLog(false, false);
                File.AppendAllText(log, "raw\n");
                var deadline = DateTime.UtcNow.AddSeconds(5);
                byte[] speech;
                while ((speech = host.DrainSpeech()).Length == 0 && DateTime.UtcNow < deadline) Thread.Sleep(10);
                Check(Encoding.UTF8.GetString(speech) == "88");
            }
            Check(File.Exists(path));
            using (var restarted = new HostEnvironment(directory.FullName))
            {
                var loaded = ActGlobals.oFormActMain!.CustomTriggers.Values.Single();
                Check(loaded.SoundData == "${value}" && loaded.Timer && loaded.Active && loaded.LastAudioAlert == DateTime.MinValue);
            }
            var saved = File.ReadAllText(path);
            var failedSave = new HostEnvironment(directory.FullName);
            ActGlobals.oFormActMain!.CustomTriggers.Values.Single().ShortRegexString = "[";
            try { failedSave.Dispose(); throw new Exception("Invalid edited regex was persisted"); }
            catch (AggregateException error) { Check(error.InnerExceptions.Any(exception => exception is RegexParseException)); }
            Check(ActGlobals.oFormActMain is null && File.ReadAllText(path) == saved && Directory.GetFiles(directory.FullName, "*.tmp").Length == 0);
            File.WriteAllText(path, "<CustomTriggers><Trigger Regex='[' /></CustomTriggers>");
            try { using var invalid = new HostEnvironment(directory.FullName); throw new Exception("Invalid regex was accepted"); }
            catch (RegexParseException) { }
            Check(ActGlobals.oFormActMain is null && File.ReadAllText(path).Contains("Regex='['"));
            File.WriteAllText(path, "<!DOCTYPE CustomTriggers [<!ENTITY x SYSTEM 'file:///etc/hostname'>]><CustomTriggers/>");
            try { using var invalid = new HostEnvironment(directory.FullName); throw new Exception("Trigger DTD was accepted"); }
            catch (XmlException) { }
            File.WriteAllText(path, saved);
            using var recovered = new HostEnvironment(directory.FullName);
            Check(ActGlobals.oFormActMain!.CustomTriggers.Count == 1);
            Console.WriteLine("Custom trigger routing, captures, import isolation, timeout and restart persistence passed.");
        }
        finally { directory.Delete(true); }
    }
}
