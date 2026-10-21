using System.Reflection;
using System.Runtime.CompilerServices;
using System.Windows.Forms;
using Advanced_Combat_Tracker;

namespace Aftercast.LegacyReference;

public static class AudioProbe
{
    public static string Run()
    {
        var previous = ActGlobals.oFormActMain;
        var main = (FormActMain)RuntimeHelpers.GetUninitializedObject(typeof(FormActMain));
        var timers = (FormSpellTimers)RuntimeHelpers.GetUninitializedObject(typeof(FormSpellTimers));
        GC.SuppressFinalize(main); GC.SuppressFinalize(timers);
        const BindingFlags fields = BindingFlags.Instance | BindingFlags.NonPublic | BindingFlags.Public;
        var controls = new List<IDisposable>();
        void Set(object instance, string name, object value) => instance.GetType().GetField(name, fields)?.SetValue(instance, value);
        var directory = Directory.CreateTempSubdirectory("aftercast-audio-oracle-");
        try
        {
            ActGlobals.oFormActMain = main;
            // Install only the original option controls that audio reads; never start ACT's application or device player.
            foreach (var name in new[] { "opSound", "opSoundTts" })
            {
                var field = typeof(FormActMain).GetField(name, fields);
                if (field is null) continue;
                var options = RuntimeHelpers.GetUninitializedObject(field.FieldType);
                GC.SuppressFinalize(options);
                foreach (var controlField in field.FieldType.GetFields(fields))
                {
                    if (controlField.FieldType != typeof(TrackBar) && controlField.FieldType != typeof(RadioButton) && controlField.FieldType != typeof(CheckedListBox)) continue;
                    var control = (Control)Activator.CreateInstance(controlField.FieldType)!;
                    controls.Add(control);
                    if (control is TrackBar bar) { bar.Maximum = 100; bar.Value = controlField.Name == "tbarTtsVol" ? 37 : 61; }
                    controlField.SetValue(options, control);
                }
                field.SetValue(main, options);
            }
            Set(main, "ttsLock", new object()); Set(main, "ttsAsyncMax", 50);
            Set(main, "actExe", new FileInfo(typeof(FormActMain).Assembly.Location));
            Set(main, "SoundVolume", 61); Set(main, "TtsVolume", 37);
            var translations = typeof(FormSpellTimers).GetField("Trans", fields);
            if (translations is not null) translations.SetValue(timers, Activator.CreateInstance(translations.FieldType, nonPublic: true));
            var calls = new List<string>();
            main.PlaySoundMethod = (path, volume) => calls.Add($"sound:{Path.GetFileName(path)}:{volume}");
            main.PlayTtsMethod = text => calls.Add("tts:" + text);
            var wave = Path.Combine(directory.FullName, "effect.wav");
            var sapi = Path.Combine(directory.FullName, "sapi.wav");
            File.WriteAllBytes(wave, [0]); File.WriteAllBytes(sapi, [0]);
            main.PlaySound(wave); main.PlaySound(sapi);
            var method = typeof(FormSpellTimers).GetMethod("TimerSound", fields | BindingFlags.Static)!;
            var stateType = method.GetParameters()[1].ParameterType;
            var frame = new TimerFrame("Caster", new TimerData("Fixture", "Audio"));
            void Cue(int state, string cue)
            {
                try { method.Invoke(timers, method.GetParameters().Length == 3 ? [frame, stateType.IsEnum ? Enum.ToObject(stateType, state) : state, cue] : [frame, state, cue, null]); }
                catch (Exception error) { throw new Exception($"Timer cue {state}: {cue}", error); }
            }
            Cue(0, "tts custom start"); Cue(1, "tts custom warning"); Cue(0, "none"); Cue(0, wave);
            Cue(0, "tts"); Cue(1, "tts");
            var expected = new[] { "sound:effect.wav:61", "sound:sapi.wav:37", "tts: custom start", "tts: custom warning", "sound:effect.wav:61", "tts: Fixture started", "tts: Fixture warning" };
            if (!calls.SequenceEqual(expected)) throw new Exception("Audio oracle mismatch: " + string.Join(" | ", calls));
            return "Audio";
        }
        finally
        {
            ActGlobals.oFormActMain = previous;
            foreach (var control in controls) control.Dispose();
            directory.Delete(true);
        }
    }
}
