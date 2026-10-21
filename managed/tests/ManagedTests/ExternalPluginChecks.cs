using System.Diagnostics;
using System.Security.Cryptography;
using Advanced_Combat_Tracker;
using Aftercast.Bridge;

internal static class ExternalPluginChecks
{
    internal static void Run(string assemblyPath, string typeName, string statePath, Action<PluginSession, IActPluginV1>? exercise = null)
    {
        Platform.Prepare();
        var path = Path.GetFullPath(assemblyPath);
        var hash = Convert.ToHexStringLower(SHA256.HashData(File.ReadAllBytes(path)));
        Console.WriteLine($"External plugin: {typeName}; SHA-256: {hash}");
        Directory.CreateDirectory(statePath);
        var session = new PluginSession();
        session.Configure(Path.GetFullPath(statePath));
        try
        {
            var instance = session.Construct(path, typeName);
            var id = session.Add(path, instance);
            Console.WriteLine("Constructed");
            Console.WriteLine("InitPlugin status: " + session.Initialize(id));
            var elapsed = Stopwatch.StartNew();
            // Service startup callbacks, e.g. a plugin timer, without claiming parser or visible UI readiness.
            while (elapsed.Elapsed < TimeSpan.FromMilliseconds(1500))
            {
                session.Pump();
                Thread.Sleep(10);
            }
            exercise?.Invoke(session, (IActPluginV1)instance);
            session.Deinitialize(id);
            if (session.DiagnosticsSnapshot.Item1 != 0)
                throw new Exception("External plugin diagnostics: " + string.Join("\n", session.DiagnosticsSnapshot.Item2));
            Console.WriteLine("DeInitPlugin completed; zero host diagnostics");
        }
        finally { session.Stop(); }
        Console.WriteLine("Session stopped");
    }
}
