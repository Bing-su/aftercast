using Advanced_Combat_Tracker;
using Aftercast.ActCompat;

internal static class SpeechRuleChecks
{
    private static void Check(bool value, string message) { if (!value) throw new InvalidOperationException(message); }
    private static void Reject(Action action)
    {
        try { action(); }
        catch (Exception error) when (error is ArgumentException or InvalidOperationException or IOException or InvalidDataException or UnauthorizedAccessException or System.Text.Json.JsonException) { return; }
        throw new Exception("Expected speech rule rejection");
    }
    internal static void Run()
    {
        var directory = Directory.CreateTempSubdirectory("aftercast-speech-rules-");
        try
        {
            using (var host = new HostEnvironment(directory.FullName))
            {
                var root = ActGlobals.oFormActMain!;
                var heard = new List<string>();
                root.PlayTtsMethod = value => heard.Add(value);
                var effects = new List<(string Source, int Volume)>();
                root.PlaySoundMethod = (source, volume) => effects.Add((source, volume));
                host.ConfigureAudio(61, 37, ["none", "none", "none"]);
                Check(effects.Count == 0 && heard.Count == 0, "Changing audio settings emitted playback");
                var wave = Path.Combine(directory.FullName, "fake.wav");
                var sapi = Path.Combine(directory.FullName, "sapi.wav");
                File.WriteAllText(wave, "synthetic"); File.WriteAllText(sapi, "synthetic");
                root.PlaySound(wave); root.PlaySound(sapi);
                Check(effects.SequenceEqual([(wave, 61), (sapi, 37)]), "Plugin sound delegate did not receive the correct configured volumes");
                var rules = new[] { new SpeechRule("(cat)", "$1dog", true), new SpeechRule("DOG", "hound", true), new SpeechRule("hound", "bad", false) };
                var original = root.ReadSpeechRules();
                Check(original.Rules.Length == 0, "Unexpected default rules");
                var blocked = Path.Combine(directory.FullName, "SpeechCorrections.json");
                Directory.CreateDirectory(blocked);
                Reject(() => root.SaveSpeechRules(original.Revision, rules));
                Check(root.ReadSpeechRules().Rules.Length == 0, "Failed save published rules");
                Directory.Delete(blocked);
                var saved = root.SaveSpeechRules(original.Revision, rules);
                Check(heard.Count == 0, "Saving rules played speech");
                root.TTS("CAT");
                Check(heard.SequenceEqual(["CAThound"]), "Speech rules did not preserve order, case or replacement captures for plugin delegate");
                var locale = root.SaveSpeechRules(saved.Revision, [new SpeechRule("i", "x", true)]);
                var culture = System.Globalization.CultureInfo.CurrentCulture;
                try { System.Globalization.CultureInfo.CurrentCulture = System.Globalization.CultureInfo.GetCultureInfo("tr-TR"); root.TTS("İ"); }
                finally { System.Globalization.CultureInfo.CurrentCulture = culture; }
                Check(heard.SequenceEqual(["CAThound", "x"]), "Speech matching did not use the calling thread culture");
                heard.RemoveAt(1);
                saved = root.SaveSpeechRules(locale.Revision, rules);
                Reject(() => root.SaveSpeechRules(saved.Revision, [new SpeechRule("[", "x", true)]));
                Reject(() => root.SaveSpeechRules(saved.Revision, [new SpeechRule("a▶b", "x", true)]));
                Reject(() => root.SaveSpeechRules(original.Revision, rules));
                var path = Path.Combine(directory.FullName, "SpeechCorrections.json");
                var bytes = File.ReadAllBytes(path);
                File.WriteAllText(path, "[]");
                Reject(() => root.SaveSpeechRules(saved.Revision, rules));
                Check(File.ReadAllText(path) == "[]", "External edit was overwritten");
                File.WriteAllBytes(path, bytes);
                var explosive = root.SaveSpeechRules(saved.Revision, [new SpeechRule("a", new string('b', 1024), true)]);
                root.TTS(new string('a', 5));
                Check(heard.Count == 1, "Oversized correction reached plugin delegate");
                var slow = root.SaveSpeechRules(explosive.Revision, [new SpeechRule("(a+)+$", "x", true)]);
                root.TTS(new string('a', 4000) + "!");
                Check(heard.Count == 1, "Timed-out correction reached plugin delegate");
                root.SaveSpeechRules(slow.Revision, rules);
                Check(host.DrainSpeech().Length == 0, "Fake delegate unexpectedly enqueued audio");
            }
            using (var restarted = new HostEnvironment(directory.FullName))
            {
                var heard = new List<string>();
                ActGlobals.oFormActMain!.PlayTtsMethod = value => heard.Add(value);
                ActGlobals.oFormActMain.TTS("cat");
                Check(heard.SequenceEqual(["cathound"]) && restarted.ReadSpeechRules().Rules.Length == 3, "Speech rules did not survive restart");
            }
            Console.WriteLine("Speech rule order, .NET replacement, plugin delegate, bounds, stale/external edits and restart passed.");
        }
        finally { directory.Delete(true); }
    }
}
