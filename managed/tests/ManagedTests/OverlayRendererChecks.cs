using System.Reflection;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using Aftercast.Bridge;

internal static class OverlayRendererChecks
{
    internal static void Run(string path)
    {
        if (!OperatingSystem.IsWindows() || IntPtr.Size != 8) throw new PlatformNotSupportedException("Pinned renderer check needs Windows x64");
        path = Path.GetFullPath(path);
        var original = File.ReadAllBytes(path);
        var tampered = (byte[])original.Clone();
        tampered[0] ^= 1;
        try { OverlayRendererCompatibility.Rewrite(tampered); throw new Exception("Unknown renderer was rewritten"); }
        catch (InvalidOperationException) { }
        var context = new PluginContext(path, SHA256.HashData(original));
        var assembly = OverlayRendererCompatibility.LoadIfKnown(context, path)!;
        var copy = assembly.GetType("RainbowMage.HtmlRenderer.NativeMethods", true)!.GetMethod("memcpy", BindingFlags.Static | BindingFlags.Public)!;
        var source = Marshal.AllocHGlobal(32);
        var target = Marshal.AllocHGlobal(32);
        try
        {
            var bytes = Enumerable.Range(0, 32).Select(value => (byte)value).ToArray();
            Marshal.Copy(bytes, 0, source, bytes.Length);
            var result = Enumerable.Repeat((byte)0xaa, 32).ToArray();
            Marshal.Copy(result, 0, target, result.Length);
            copy.Invoke(null, new object[] { target + 8, source + 4, 9u });
            Marshal.Copy(target, result, 0, result.Length);
            if (!result.AsSpan(8, 9).SequenceEqual(bytes.AsSpan(4, 9)) || result.Take(8).Concat(result.Skip(17)).Any(value => value != 0xaa))
                throw new Exception("Native renderer copy changed data outside its destination range");
            copy.Invoke(null, new object[] { target, source, 0u });
            var afterZero = new byte[32];
            Marshal.Copy(target, afterZero, 0, afterZero.Length);
            if (!result.SequenceEqual(afterZero)) throw new Exception("Zero-length renderer copy changed data");
        }
        finally { Marshal.FreeHGlobal(target); Marshal.FreeHGlobal(source); }
        if (!File.ReadAllBytes(path).SequenceEqual(original)) throw new Exception("Renderer compatibility changed the source DLL");
        Console.WriteLine("Pinned renderer copy, buffer bounds, zero length, unknown-hash rejection and original-file preservation passed.");
    }
}
