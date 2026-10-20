using System.Reflection;
using System.Reflection.Metadata;
using System.Reflection.Metadata.Ecma335;
using System.Reflection.PortableExecutable;
using System.Runtime.ExceptionServices;
using System.Runtime.InteropServices;
using System.Runtime.Loader;
using System.Security.Cryptography;
using System.Text;
using Advanced_Combat_Tracker;
using Bxivact.ActCompat;
using Bxivact.Bridge;

internal static class Program
{
    private static void Check(bool condition)
    {
        if (!condition) throw new Exception("Managed contract check failed");
    }
    private static string Diagnostic()
    {
        var output = Marshal.AllocHGlobal(8192);
        try
        {
            var length = EntryPoint.ReadError(output, 8192);
            Check(length >= 0 && length <= 8192);
            return Marshal.PtrToStringUTF8(output, length)!;
        }
        finally { Marshal.FreeHGlobal(output); }
    }
    public static void Main(string[] args)
    {
        // Only the original-reference test calls ACT UI types before startup; ABI modes exercise normal binding.
        if (OperatingSystem.IsLinux() && args.Single() == "act")
        {
            AssemblyLoadContext.Default.LoadFromAssemblyPath(Path.Combine(AppContext.BaseDirectory, "System.Windows.Forms.dll"));
            AssemblyLoadContext.Default.LoadFromAssemblyPath(Path.Combine(AppContext.BaseDirectory, "System.Windows.Forms.DataVisualization.dll"));
        }
        // Modes keep artifact-dependent checks separate from ordinary ABI tests without a test framework dependency.
        switch (args.Single())
        {
            case "bridge": Bridge(); break;
            case "plugins": PluginRequests(); break;
            case "prepare-act": PrepareAct(); break;
            case "act": ActBinding(); break;
            case "linux": LinuxCompatibility(); break;
            default: throw new ArgumentException("Unknown managed test mode");
        }
        Console.WriteLine($"Managed {args[0]} contract checks passed.");
    }
    private static void Bridge()
    {
        var buffer = Marshal.AllocHGlobal(24);
        try
        {
            Check(EntryPoint.Invoke(0, 24) == 1);
            Check(EntryPoint.Invoke(buffer, 23) == 1);
            Check(EntryPoint.ConfigureHost(0, 40) == -1);
            Check(EntryPoint.CreatePlugin(0, 40) == -1);
            Check(EntryPoint.PluginCommand(0, 8224) == -1);
            Marshal.WriteInt32(buffer, 0, 24);
            Marshal.WriteInt32(buffer, 4, 2);
            Check(EntryPoint.Invoke(buffer, 24) == 2);
            Marshal.WriteInt32(buffer, 4, 1);
            Marshal.WriteInt32(buffer, 8, 99);
            Marshal.WriteInt64(buffer, 16, 0);
            Check(EntryPoint.Invoke(buffer, 24) == 6);
            Check(EntryPoint.ReadError(0, 0) == -1);
            var expected = "Unknown bridge command: 99";
            var required = EntryPoint.ReadError(buffer, 1);
            Check(required == Encoding.UTF8.GetByteCount(expected));
            Check(Marshal.ReadInt32(buffer, 0) == 24);
            Check(Diagnostic() == expected);
            Marshal.WriteInt32(buffer, 8, 2);
            Check(EntryPoint.Invoke(buffer, 24) == 4);
            Marshal.WriteInt32(buffer, 8, 1);
            Check(EntryPoint.Invoke(buffer, 24) == 0);
            Check(EntryPoint.ReadError(buffer, 0) == 0);
            var session = Marshal.ReadInt64(buffer, 16);
            Check(session > 0 && Marshal.ReadInt32(buffer, 12) == 10);
            Check(EntryPoint.Invoke(buffer, 24) == 3);
            Marshal.WriteInt32(buffer, 8, 3);
            Marshal.WriteInt64(buffer, 16, session + 1);
            Check(EntryPoint.Invoke(buffer, 24) == 5);
            Marshal.WriteInt64(buffer, 16, session);
            Check(EntryPoint.Invoke(buffer, 24) == 0);
            Check(EntryPoint.Invoke(buffer, 24) == 4);
            // Neither runtime assembly may silently regain a compiler runtime dependency.
            foreach (var assembly in new[] { typeof(EntryPoint).Assembly, typeof(Dnum).Assembly })
                Check(assembly.GetReferencedAssemblies().All(reference => reference.Name != "FSharp.Core"));
        }
        finally { Marshal.FreeHGlobal(buffer); }
    }
    private static void PluginRequests()
    {
        var memory = Marshal.AllocHGlobal(65537);
        try
        {
            Marshal.WriteInt32(memory, 0, 24);
            Marshal.WriteInt32(memory, 4, 1);
            Marshal.WriteInt32(memory, 8, 1);
            Check(EntryPoint.Invoke(memory, 24) == 0);
            var session = Marshal.ReadInt64(memory, 16);
            var path = Encoding.UTF8.GetBytes(Path.GetFullPath("managed/tests/PluginFixture/bin/Release/net10.0/Bxivact.PluginFixture.dll"));
            var name = Encoding.UTF8.GetBytes("Bxivact.PluginFixture.GoodPlugin");
            var size = 40 + path.Length + name.Length;
            void Reset()
            {
                Marshal.WriteInt32(memory, 0, size);
                Marshal.WriteInt32(memory, 4, 1);
                Marshal.WriteInt64(memory, 8, session);
                Marshal.WriteInt32(memory, 16, 40);
                Marshal.WriteInt32(memory, 20, path.Length);
                Marshal.WriteInt32(memory, 24, 40 + path.Length);
                Marshal.WriteInt32(memory, 28, name.Length);
                Marshal.WriteInt64(memory, 32, 0);
                Marshal.Copy(path, 0, memory + 40, path.Length);
                Marshal.Copy(name, 0, memory + 40 + path.Length, name.Length);
            }
            void Rejected(Action change, string expected)
            {
                Reset();
                change();
                Check(EntryPoint.CreatePlugin(memory, size) == -1);
                Check(Diagnostic().Contains(expected));
                Check(Marshal.ReadInt64(memory, 32) == 0);
            }
            Check(EntryPoint.CreatePlugin(0, 40) == -1);
            Check(EntryPoint.CreatePlugin(memory, 39) == -1);
            Check(EntryPoint.CreatePlugin(memory, 65537) == -1);
            Rejected(() => Marshal.WriteInt32(memory, 4, 2), "size/version");
            Rejected(() => Marshal.WriteInt64(memory, 8, session + 1), "does not own");
            Rejected(() => Marshal.WriteInt32(memory, 16, int.MaxValue), "outside");
            Rejected(() => Marshal.WriteInt32(memory, 20, int.MaxValue), "outside");
            Rejected(() => Marshal.WriteInt32(memory, 28, -1), "outside");
            Rejected(() => Marshal.WriteInt32(memory, 24, 0), "outside");
            Rejected(() => Marshal.WriteByte(memory, 40, 0), "NUL");
            Rejected(() => Marshal.WriteByte(memory, 40, 255), "translate");
            Reset();
            Check(EntryPoint.CreatePlugin(memory, size) == 0);
            var instance = Marshal.ReadInt64(memory, 32);
            Check(instance > 0 && Diagnostic() == "");
            void Command()
            {
                Marshal.WriteInt32(memory, 0, 8224);
                Marshal.WriteInt32(memory, 4, 1);
                Marshal.WriteInt64(memory, 8, session);
                Marshal.WriteInt64(memory, 16, instance);
                Marshal.WriteInt32(memory, 24, 99);
                Marshal.WriteInt32(memory, 28, 0);
            }
            Check(EntryPoint.PluginCommand(0, 8224) == -1);
            Check(EntryPoint.PluginCommand(memory, 8223) == -1);
            Check(EntryPoint.PluginCommand(memory, 8225) == -1);
            Command();
            Marshal.WriteInt32(memory, 4, 2);
            Check(EntryPoint.PluginCommand(memory, 8224) == -1 && Diagnostic().Contains("size/version"));
            Command();
            Marshal.WriteInt64(memory, 8, session + 1);
            Check(EntryPoint.PluginCommand(memory, 8224) == -1 && Diagnostic().Contains("does not own"));
            Command();
            Check(EntryPoint.PluginCommand(memory, 8224) == -1 && Diagnostic().Contains("Unknown plugin lifecycle command"));
            Command();
            Marshal.WriteInt32(memory, 24, 1);
            Marshal.WriteInt64(memory, 16, long.MaxValue);
            Check(EntryPoint.PluginCommand(memory, 8224) == -1 && Diagnostic().Contains("Unknown plugin instance"));
            Marshal.WriteInt32(memory, 0, 24);
            Marshal.WriteInt32(memory, 4, 1);
            Marshal.WriteInt32(memory, 8, 3);
            Marshal.WriteInt64(memory, 16, session);
            Check(EntryPoint.Invoke(memory, 24) == 0);
            Reset();
            Check(EntryPoint.CreatePlugin(memory, size) == -1 && Diagnostic().Contains("No managed ACT session"));
        }
        finally { Marshal.FreeHGlobal(memory); }
    }
    private static void PrepareAct()
    {
        using var input = File.OpenRead("binaries/act/Advanced Combat Tracker.exe");
        Check(Convert.ToHexStringLower(SHA256.HashData(input)) == "934fc00d0a3c8325b0f7445358860b4c013c5dab3d8fff012512acf6502dd657");
        input.Position = 0;
        using var pe = new PEReader(input);
        var metadata = pe.GetMetadataReader();
        var assembly = metadata.GetAssemblyDefinition();
        Check(metadata.GetString(assembly.Name) == "Advanced Combat Tracker" && assembly.Version == new Version(3, 8, 5, 288));
        Check(File.ReadAllBytes("managed/ActCompat/ACT.publickey").AsSpan().SequenceEqual(metadata.GetBlobBytes(assembly.PublicKey)));
    }
    private static void ActBinding()
    {
        // Load only the compiled original-reference probe; never execute the supplied ACT EXE.
        var legacy = Assembly.LoadFrom(Path.GetFullPath("managed/tests/LegacyReference/bin/Release/net10.0/Bxivact.LegacyReference.dll"));
        const string identity = "Advanced Combat Tracker, Version=3.8.5.288, Culture=neutral, PublicKeyToken=a946b61e93d97868";
        Check(legacy.GetReferencedAssemblies().Single(item => item.Name == "Advanced Combat Tracker").FullName == identity);
        var actual = typeof(Dnum).Assembly;
        Check(actual.FullName == identity);
        Check(SHA256.HashData(File.ReadAllBytes(actual.Location)).AsSpan().SequenceEqual(
            SHA256.HashData(File.ReadAllBytes("crates/bxivact/managed/Advanced Combat Tracker.dll"))));
        Check(!typeof(Dnum).IsValueType && !typeof(Dnum).IsSealed);
        object? Call(string type, string name, params object[] arguments)
        {
            try { return legacy.GetType("Bxivact.LegacyReference." + type)!.GetMethod(name)!.Invoke(null, arguments); }
            catch (TargetInvocationException error) when (error.InnerException is not null)
            { ExceptionDispatchInfo.Capture(error.InnerException).Throw(); throw; }
        }
        Check((long)Call("ContractProbe", "Run")! == 123);
        Check((string)Call("ChartProbe", "Run")! == "Network");
        Check((string)Call("ListViewProbe", "Run")! == "42");
        Check(typeof(Dnum).GetProperty("Number")!.GetSetMethod() is null && new Dnum(-11).Number == -11);
        try { new Dnum(1).ToString(); throw new Exception("Unimplemented formatting succeeded"); }
        catch (NotSupportedException) { }
        var directory = Directory.CreateTempSubdirectory("bxivact-contract-");
        try
        {
            Check(Call("HostProbe", "Root") is null);
            using (var environment = new HostEnvironment(directory.FullName))
            {
                Check(Call("HostProbe", "Root") is not null);
                Check((string)Call("HostProbe", "Character", "Example Character")! == "Example Character");
                Check((string)Call("HostProbe", "AppDataPath")! == directory.FullName);
                Check((bool)Call("HostProbe", "MissingRegistration")!);
                Call("HostProbe", "BackgroundDiagnostic");
                var (count, diagnostic) = environment.Diagnostics;
                Check(count == 1 && diagnostic.Contains("background diagnostic") && environment.Errors.Item1 == 0);
                try { Call("HostProbe", "Restart"); throw new Exception("Embedded restart succeeded"); }
                catch (PlatformNotSupportedException) { }
            }
            Check(Call("HostProbe", "Root") is null);
        }
        finally { directory.Delete(true); }
    }
    private static void LinuxCompatibility()
    {
        var path = "binaries/FFXIV_ACT_Plugin_SDK_3.0.3.3/SDK\\FFXIV_ACT_Plugin.Memory.dll";
        var original = File.ReadAllBytes(path);
        var before = SHA256.HashData(original);
        var rewritten = LinuxPluginCompatibility.RewriteMemory(original);
        Check(SHA256.HashData(original).AsSpan().SequenceEqual(before));
        Check(SHA256.HashData(File.ReadAllBytes(path)).AsSpan().SequenceEqual(before));
        Check(rewritten.Length == original.Length && !ReferenceEquals(original, rewritten));
        using var stream = new MemoryStream(rewritten, false);
        using var pe = new PEReader(stream);
        var metadata = pe.GetMetadataReader();
        Check(metadata.GetGuid(metadata.GetModuleDefinition().Mvid) == new Guid("89b13c66-df57-43bf-8217-c84f5c77f5f6"));
        var method = metadata.GetMethodDefinition(MetadataTokens.MethodDefinitionHandle(0x96));
        Check(metadata.GetString(method.Name) == "SetIsAdministratorFlag");
        var il = pe.GetMethodBody(method.RelativeVirtualAddress).GetILBytes()!;
        Check(Convert.ToHexString(il.AsSpan(0, 6)) == "02288E000006" && il[6] == 0x16);
        Check(il[7..26].All(value => value == 0) && Convert.ToHexString(il.AsSpan(26, 6)) == "6F820000062A");
        for (var index = 0; index < original.Length; index++)
            if (index < 9743 || index > 9762) Check(original[index] == rewritten[index]);
        void Rejected(byte[] bytes)
        {
            try { LinuxPluginCompatibility.RewriteMemory(bytes); throw new Exception("Unrecognized assembly was rewritten"); }
            catch (InvalidOperationException error) { Check(error.Message.Contains("Unsupported FFXIV memory assembly")); }
        }
        Rejected([]);
        Rejected(rewritten);
        var changed = (byte[])original.Clone();
        changed[100] ^= 1;
        Rejected(changed);
        Check(!LinuxPluginCompatibility.IsKnownPlugin(new byte[32]));
        var main = File.ReadAllBytes("binaries/FFXIV_ACT_Plugin_SDK_3.0.3.3/FFXIV_ACT_Plugin.dll");
        Check(LinuxPluginCompatibility.IsKnownPlugin(SHA256.HashData(main)) == OperatingSystem.IsLinux());
    }
}
