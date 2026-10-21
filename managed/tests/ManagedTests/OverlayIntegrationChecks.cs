using System.Collections;
using System.Diagnostics;
using System.Net;
using System.Net.Sockets;
using System.Reflection;
using System.Text.Json;
using Advanced_Combat_Tracker;
using Aftercast.Bridge;

// Use vendor public APIs for fixture state and real log following/WS handlers for the browser; no private callback is invoked.
internal static class OverlayIntegrationChecks
{
    internal static void Run(PluginSession session)
    {
        var root = ActGlobals.oFormActMain!;
        var overlay = root.ActPlugins.Single(data => data.pluginObj.GetType().FullName == "RainbowMage.OverlayPlugin.PluginLoader");
        if (!root.InitActDone || overlay.lblPluginStatus.Text != "Initialised") throw new Exception("OverlayPlugin startup did not complete");
        session.ConfigureSpeech(true);
        session.ConfigureSound(true);
        var container = overlay.pluginObj.GetType().GetProperty("Container")!.GetValue(overlay.pluginObj)!;
        object Resolve(string typeName)
        {
            var type = AppDomain.CurrentDomain.GetAssemblies().Select(assembly => assembly.GetType(typeName)).First(type => type is not null)!;
            return container.GetType().GetMethod("Resolve", new[] { typeof(Type) })!.Invoke(container, new object[] { type })!;
        }
        var registry = Resolve("RainbowMage.OverlayPlugin.Registry");
        var sources = ((IEnumerable)registry.GetType().GetProperty("EventSources")!.GetValue(registry)!).Cast<object>().ToArray();
        var cactbot = sources.Single(source => source.GetType().FullName == "Cactbot.CactbotEventSource");
        // Its one-shot polling timer can report disabled between polls; the installed log subscription proves Start ran.
        var logHandlers = (Delegate?)typeof(FormActMain).GetField("OnLogLineRead", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(root);
        if (logHandlers?.GetInvocationList().Any(handler => ReferenceEquals(handler.Target, cactbot)) != true)
            throw new Exception("Cactbot event source did not subscribe to ACT logs");
        var config = Resolve("RainbowMage.OverlayPlugin.IPluginConfig");
        using var reservation = new TcpListener(IPAddress.Loopback, 0);
        reservation.Start();
        var port = ((IPEndPoint)reservation.LocalEndpoint).Port;
        reservation.Stop();
        config.GetType().GetProperty("WSServerIP")!.SetValue(config, "127.0.0.1");
        config.GetType().GetProperty("WSServerPort")!.SetValue(config, port);
        config.GetType().GetProperty("WSServerSSL")!.SetValue(config, false);
        var server = Resolve("RainbowMage.OverlayPlugin.WSServer");
        server.GetType().GetMethod("Start")!.Invoke(server, null);
        if (!(bool)server.GetType().GetMethod("IsRunning")!.Invoke(server, null)!) throw new Exception("Original WS server did not start");
        var state = root.AppDataFolder.FullName;
        var log = Path.Combine(state, "integration-live.log");
        File.WriteAllText(log, "");
        root.LogFilePath = log;
        root.OpenLog(false, false);
        var commandPath = Path.Combine(state, "browser-command.json");
        var ackPath = Path.Combine(state, "browser-ack.json");
        File.Delete(commandPath); File.Delete(ackPath);
        File.WriteAllText(Path.Combine(state, "integration-ready.json"), JsonSerializer.Serialize(new { wsUrl = $"ws://127.0.0.1:{port}/ws" }));
        var cactbotPath = root.ActPlugins.Single(data => data.pluginObj.GetType().FullName == "Cactbot.PluginLoader").pluginFile.DirectoryName!;
        var core = server.GetType().Assembly;
        var nativeConfig = Activator.CreateInstance(core.GetType("RainbowMage.OverlayPlugin.Overlays.MiniParseOverlayConfig")!, "aftercast integration")!;
        nativeConfig.GetType().GetProperty("Url")!.SetValue(nativeConfig, new Uri(Path.Combine(cactbotPath, "ui", "raidboss", "raidboss.html")).AbsoluteUri + "?timeline=1&alerts=1&audio=0");
        nativeConfig.GetType().GetProperty("IsVisible")!.SetValue(nativeConfig, false);
        nativeConfig.GetType().GetProperty("IsLocked")!.SetValue(nativeConfig, true);
        nativeConfig.GetType().GetProperty("Size")!.SetValue(nativeConfig, new System.Drawing.Size(800, 500));
        using var native = (IDisposable)Activator.CreateInstance(core.GetType("RainbowMage.OverlayPlugin.Overlays.MiniParseOverlay")!, nativeConfig, "aftercast integration", container)!;
        native.GetType().GetMethod("Start")!.Invoke(native, null);
        var nativeForm = native.GetType().GetProperty("Overlay")!.GetValue(native) ?? throw new Exception("Original CEF overlay construction failed");
        var sourceConfig = cactbot.GetType().GetProperty("Config")!.GetValue(cactbot)!;
        var overlayData = (IDictionary)sourceConfig.GetType().GetField("OverlayData")!.GetValue(sourceConfig)!;
        overlayData.Remove("aftercast-cef-inspection");
        var snapshots = new List<JsonElement>();
        CheckCef(0, snapshot => snapshot.GetProperty("loaded").GetBoolean());
        // Exercise the original CEF/cactbot handler; the Rust backend is verified independently by the host fixture test.
        native.GetType().GetMethod("ExecuteScript")!.Invoke(native, new object[] { "callOverlayHandler({call:'cactbotSay',text:'Aftercast CEF speech fixture'})" });
        var soundFile = Path.Combine(state, "aftercast-effect.wav");
        // The transport probe records requests; the Rust device test independently decodes/plays a valid WAV.
        File.WriteAllBytes(soundFile, [0]);
        File.WriteAllText(Path.Combine(state, "sound-ready.json"), JsonSerializer.Serialize(new { file = soundFile }));
        native.GetType().GetMethod("ExecuteScript")!.Invoke(native, new object[] { "callOverlayHandler({call:'playSound',file:" + JsonSerializer.Serialize(soundFile) + "})" });
        var start = new ProcessStartInfo("node") { UseShellExecute = false, CreateNoWindow = true, RedirectStandardOutput = true, RedirectStandardError = true };
        foreach (var argument in new[] { Path.GetFullPath("managed/tests/cactbot-browser.cjs"), cactbotPath, Path.Combine(state, "browser"), state }) start.ArgumentList.Add(argument);
        using var child = Process.Start(start)!;
        var stdout = child.StandardOutput.ReadToEndAsync();
        var stderr = child.StandardError.ReadToEndAsync();
        var elapsed = Stopwatch.StartNew();
        var lastCommand = 0;
        try
        {
            while (!child.HasExited)
            {
                if (elapsed.Elapsed > TimeSpan.FromSeconds(60)) throw new TimeoutException("Full-stack browser verification timed out");
                session.Pump();
                if (File.Exists(commandPath))
                {
                    // Share deletion so Node can atomically replace the next command while Windows closes its rename handle.
                    using var commandFile = new FileStream(commandPath, FileMode.Open, FileAccess.Read, FileShare.ReadWrite | FileShare.Delete);
                    using var command = JsonDocument.Parse(commandFile);
                    var id = command.RootElement.GetProperty("id").GetInt32();
                    if (id > lastCommand)
                    {
                        var kind = command.RootElement.GetProperty("kind").GetString();
                        if (kind == "state")
                        {
                            var assembly = cactbot.GetType().Assembly;
                            var entityType = assembly.GetType("Cactbot.FFXIVProcess+EntityData")!;
                            var entity = Activator.CreateInstance(entityType)!;
                            entityType.GetField("name")!.SetValue(entity, "Fixture Player");
                            entityType.GetField("id")!.SetValue(entity, 0x10000001u);
                            entityType.GetField("level")!.SetValue(entity, (short)100);
                            entityType.GetField("hp")!.SetValue(entity, 1000);
                            entityType.GetField("max_hp")!.SetValue(entity, 1000);
                            var job = entityType.GetField("job")!;
                            job.SetValue(entity, Enum.Parse(job.FieldType, "PLD"));
                            var player = Activator.CreateInstance(assembly.GetType("Cactbot.JSEvents+PlayerChangedEvent")!, entity)!;
                            cactbot.GetType().GetMethod("DispatchToJS")!.Invoke(cactbot, new[] { player });
                            Append("01", "0086|Middle La Noscea");
                            Append("00", "0038||cactbot test response");
                        }
                        else if (kind == "start") Append("00", "001D||You bow courteously to the striking dummy.");
                        else if (kind == "jump") Append("00", "0038||test sync2");
                        else if (kind == "reset") Append("00", "001D||You bid farewell to the striking dummy.");
                        else throw new ArgumentException("Unknown browser fixture command");
                        CheckCef(id, snapshot => kind switch {
                            "state" => snapshot.GetProperty("popups").EnumerateArray().Select(value => value.GetString()).SequenceEqual(new[] { "1", "2", "3" }),
                            "start" => snapshot.GetProperty("bars").EnumerateArray().Any(value => value.GetString() == "Angry Dummy"),
                            "jump" => snapshot.GetProperty("bars").EnumerateArray().Any(value => value.GetString() == "Three"),
                            "reset" => snapshot.GetProperty("bars").GetArrayLength() == 0,
                            _ => false
                        });
                        if (kind == "start")
                        {
                            // Screenshot waits for a paint; keep pumping so the CEF paint thread's UI Invoke can finish.
                            var capture = Task.Run(() => native.GetType().GetMethod("Screenshot")!.Invoke(native, null));
                            var captureWait = Stopwatch.StartNew();
                            while (!capture.IsCompleted && captureWait.Elapsed < TimeSpan.FromSeconds(10)) { session.Pump(); Thread.Sleep(10); }
                            if (!capture.IsCompleted) throw new TimeoutException("Original CEF frame capture timed out");
                            using var frame = (IDisposable)(capture.GetAwaiter().GetResult()
                                ?? throw new Exception("Original CEF renderer did not produce a frame"));
                            frame.GetType().GetMethod("Save", new[] { typeof(string) })!.Invoke(frame,
                                new object[] { Path.Combine(state, "cef-raidboss.png") });
                        }
                        lastCommand = id;
                        File.WriteAllText(ackPath, JsonSerializer.Serialize(new { id }));
                    }
                }
                Thread.Sleep(10);
            }
            File.WriteAllText(Path.Combine(state, "browser-stdout.txt"), stdout.GetAwaiter().GetResult());
            var errors = stderr.GetAwaiter().GetResult();
            File.WriteAllText(Path.Combine(state, "browser-stderr.txt"), errors);
            if (child.ExitCode != 0) throw new Exception("Full-stack browser verification failed: " + errors);
            if (lastCommand != 4 || session.DiagnosticsSnapshot.Item1 != 0) throw new Exception("Incomplete live-log fixture or host errors");
            var speech = new List<string>();
            byte[] spoken;
            while ((spoken = session.DrainSpeech()).Length != 0) speech.Add(System.Text.Encoding.UTF8.GetString(spoken));
            if (speech.Count != 2 || !speech.Contains("Aftercast CEF speech fixture") || !speech.Contains("Aftercast WS speech fixture"))
                throw new Exception("Original cactbotSay did not route CEF/WS requests to ACT TTS");
            File.WriteAllText(Path.Combine(state, "tts-route.json"), JsonSerializer.Serialize(speech));
            var sounds = new List<string>();
            Tuple<byte[], int> sound;
            while ((sound = session.DrainSound()).Item1.Length != 0)
            {
                if (sound.Item2 != 100) throw new Exception("Overlay sound volume changed");
                sounds.Add(System.Text.Encoding.UTF8.GetString(sound.Item1));
            }
            if (sounds.Count != 2 || sounds.Any(source => source != soundFile)) throw new Exception("Original CEF/WS playSound did not reach ACT");
            File.WriteAllText(Path.Combine(state, "sound-route.json"), JsonSerializer.Serialize(sounds));
            var pluginLog = string.Join("\n", overlay.tpPluginSpace.Controls.Find("logBox", true).Select(control => control.Text));
            File.WriteAllText(Path.Combine(state, "overlay-integration.log"), pluginLog);
            var unexpected = pluginLog.Split('\n').Where(line =>
                line.Contains("Error:") && !line.Contains("Requires .NET 4.6 or above. Using 10.") ||
                line.Contains("Could not init FFXIV integration") || line.Contains("Could not construct"));
            if (unexpected.Any()) throw new Exception("Original plugin reported integration errors: " + string.Join("\n", unexpected.Take(4)));
            File.WriteAllText(Path.Combine(state, "cef-report.json"), JsonSerializer.Serialize(snapshots));
            Console.WriteLine("Original CEF overlay and WS/Edge both passed cactbot handlers, public player state and live-log raidboss triggers/timeline.");
        }
        finally
        {
            if (!child.HasExited) { child.Kill(entireProcessTree: true); child.WaitForExit(); }
            server.GetType().GetMethod("Stop")!.Invoke(server, null);
        }
        native.Dispose();
        session.Stop();
        var remaining = (Delegate?)typeof(FormActMain).GetField("OnLogLineRead", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(root);
        if (ActGlobals.oFormActMain is not null || remaining?.GetInvocationList().Any(handler => ReferenceEquals(handler.Target, cactbot)) == true)
            throw new Exception("Cactbot hooks or host globals survived full-stack shutdown");
        File.WriteAllText(Path.Combine(state, "teardown-report.json"), JsonSerializer.Serialize(new { globalsCleared = true, cactbotLogHookRemoved = true }));
        void Append(string type, string body) => File.AppendAllText(log, $"{type}|{DateTime.Now:O}|{body}|fixture\n");
        void CheckCef(int probe, Func<JsonElement, bool> check)
        {
            var wait = Stopwatch.StartNew();
            while (wait.Elapsed < TimeSpan.FromSeconds(10))
            {
                // Inspect DOM through the real CEF binding and original cactbotSaveData handler, not a replacement API.
                native.GetType().GetMethod("ExecuteScript")!.Invoke(native, new object[] {
                    "(async()=>{if(typeof callOverlayHandler!=='function')return;await callOverlayHandler({call:'cactbotSaveData',overlay:'aftercast-cef-inspection',data:{probe:" + probe +
                    ",loaded:!!document.querySelector('#instructions-0')?.textContent,popups:[...document.querySelectorAll('#popup-text-container .holder')].map(e=>e.textContent),bars:[...document.querySelectorAll('#timeline timer-bar')].map(e=>e.getAttribute('lefttext'))}})})()" });
                var tick = Stopwatch.StartNew();
                while (tick.Elapsed < TimeSpan.FromMilliseconds(100)) { session.Pump(); Thread.Sleep(10); }
                if (overlayData["aftercast-cef-inspection"] is { } value)
                {
                    using var document = JsonDocument.Parse(value.ToString()!);
                    if (document.RootElement.GetProperty("probe").GetInt32() == probe && check(document.RootElement))
                    { snapshots.Add(document.RootElement.Clone()); return; }
                }
            }
            throw new Exception("Original CEF raidboss DOM check failed at probe " + probe);
        }
    }
}
