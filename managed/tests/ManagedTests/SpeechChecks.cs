using System.Runtime.InteropServices;
using System.Text;
using Advanced_Combat_Tracker;
using Aftercast.ActCompat;
using Aftercast.Bridge;

internal static class SpeechChecks
{
    internal static void Run()
    {
        var directory = Directory.CreateTempSubdirectory("aftercast-speech-");
        try
        {
            using var environment = new HostEnvironment(directory.FullName);
            var root = ActGlobals.oFormActMain!;
            root.TTS("disabled");
            if (environment.DiagnosticsSnapshot.Item1 != 1 || environment.DrainSpeech().Length != 0) throw new Exception("Disabled TTS silently accepted speech");
            environment.ConfigureSpeech(true);
            root.TTS("replace backend");
            environment.ConfigureSpeech(true);
            if (environment.DrainSpeech().Length != 0) throw new Exception("Backend replacement retained old speech");
            root.TTS("한글 <plain> & text");
            if (Encoding.UTF8.GetString(environment.DrainSpeech()) != "한글 <plain> & text") throw new Exception("TTS text was changed");
            foreach (var invalid in new[] { "", " \n", "x\0y", "\ud800", new string('x', 4097) }) root.TTS(invalid);
            if (environment.DiagnosticsSnapshot.Item1 != 6 || environment.DrainSpeech().Length != 0) throw new Exception("Invalid TTS reached the queue");
            Parallel.For(0, 32, i => root.TTS("speech " + i));
            root.TTS("overflow");
            if (environment.DiagnosticsSnapshot.Item1 != 7) throw new Exception("TTS queue is unbounded");
            var requests = new HashSet<string>();
            for (var i = 0; i < 32; i++) requests.Add(Encoding.UTF8.GetString(environment.DrainSpeech()));
            if (requests.Count != 32 || environment.DrainSpeech().Length != 0) throw new Exception("Concurrent TTS requests were lost or duplicated");
            root.TTS("discard");
            environment.ConfigureSpeech(false);
            if (environment.DrainSpeech().Length != 0) throw new Exception("Disabled TTS retained queued work");
            environment.ConfigureSpeech(true);
            root.TTS("closing");
            environment.BeginClosing();
            root.TTS("after close");
            if (environment.DrainSpeech().Length != 0 || environment.DiagnosticsSnapshot.Item1 != 8) throw new Exception("Closing TTS accepted work");
        }
        finally { directory.Delete(true); }
    }

    internal static void Abi(long session)
    {
        var memory = Marshal.AllocHGlobal(4128);
        try
        {
            void Reset(int size, int command, int argument = 0)
            {
                Marshal.Copy(new byte[4128], 0, memory, 4128);
                Marshal.WriteInt32(memory, 0, size); Marshal.WriteInt32(memory, 4, 1);
                Marshal.WriteInt64(memory, 8, session); Marshal.WriteInt32(memory, 16, command); Marshal.WriteInt32(memory, 20, argument);
            }
            void Check(bool success) { if (!success) throw new Exception("Speech ABI validation failed"); }
            Check(EntryPoint.SpeechCommand(0, 32) == -1);
            Reset(32, 1, 1); Marshal.WriteInt64(memory, 8, session + 1);
            Check(EntryPoint.SpeechCommand(memory, 32) == -1);
            Reset(32, 1, 2); Check(EntryPoint.SpeechCommand(memory, 32) == -1);
            Reset(32, 2); Check(EntryPoint.SpeechCommand(memory, 32) == -1);
            Reset(4128, 2); Marshal.WriteInt64(memory, 24, 1); Check(EntryPoint.SpeechCommand(memory, 4128) == -1);
            Reset(4128, 2); Check(EntryPoint.SpeechCommand(memory, 4127) == -1);
            Reset(32, 1, 1); Check(EntryPoint.SpeechCommand(memory, 32) == 0);
            Reset(4128, 2); Check(EntryPoint.SpeechCommand(memory, 4128) == 0 && Marshal.ReadInt32(memory, 20) == 0);
            Reset(32, 1); Check(EntryPoint.SpeechCommand(memory, 32) == 0);
        }
        finally { Marshal.FreeHGlobal(memory); }
    }
}
