using System.Runtime.InteropServices;
using System.Text;
using Bxivact.ActCompat;

namespace Bxivact.Bridge;

// Retain the standard ComponentEntryPoint delegate and existing Rust ABI throughout the migration.
public static class EntryPoint
{
    private static readonly object gate = new();
    private static HostSession? active;
    private static string lastError = "";
    // Delay control-bearing generic types until Platform.Prepare has bound WinForms on startup.
    private static readonly Lazy<PluginSession> plugins = new(() => new());
    private static string Describe(Exception error)
    {
        // A plugin can override Message; even diagnostic formatting must not escape the native boundary.
        try
        {
            var text = error.GetType().FullName + ": " + error.Message;
            return text.Length > 2048 ? text[..2048] : text;
        }
        catch { return "Managed exception (diagnostic formatting failed)"; }
    }
    private static int Run(Func<int> action)
    {
        try
        {
            lock (gate)
            {
                lastError = "";
                return action();
            }
        }
        catch (Exception error) { lastError = Describe(error); return -1; }
    }
    private static int Dispatch(nint buffer)
    {
        var command = Marshal.ReadInt32(buffer, 8);
        var session = unchecked((ulong)Marshal.ReadInt64(buffer, 16));
        Marshal.WriteInt32(buffer, 12, Environment.Version.Major);
        if (command == 1)
        {
            if (active is not null) return 3;
            Platform.Prepare();
            active = new();
            Marshal.WriteInt64(buffer, 16, unchecked((long)active.Id));
            return 0;
        }
        if (command is not (2 or 3 or 4)) return 6;
        if (active is null) return 4;
        if (active.Id != session) return 5;
        if (command == 2) return 0;
        if (command == 4)
        {
            Marshal.WriteInt32(buffer, 12, plugins.Value.Pump());
            return 0;
        }
        try { plugins.Value.Stop(); } finally { active = null; }
        return 0;
    }
    public static int Invoke(nint buffer, int sizeBytes) => Run(() =>
    {
        // Validate fixed headers before reads; the caller owns this memory for this call only.
        var status = buffer == 0 || sizeBytes != 24 ? 1
            : Marshal.ReadInt32(buffer, 0) != 24 || Marshal.ReadInt32(buffer, 4) != 1 ? 2 : Dispatch(buffer);
        if (status != 0)
            lastError = status switch
            {
                1 => "Invalid lifecycle buffer: expected a non-null 24-byte header",
                2 => "Lifecycle ABI mismatch: expected header size 24 and version 1",
                3 => "A managed ACT session is already active",
                4 => "No managed ACT session is active",
                5 => "The request does not own the active managed ACT session",
                _ => $"Unknown bridge command: {Marshal.ReadInt32(buffer, 8)}"
            };
        return status;
    });
    private static void Owner(ulong session)
    {
        if (active is null) throw new InvalidOperationException("No managed ACT session is active");
        if (session != active.Id) throw new InvalidOperationException("The request does not own the active managed ACT session");
    }
    private static (string Path, string TypeName) Request(nint buffer, int sizeBytes)
    {
        if (buffer == 0 || sizeBytes < 40 || sizeBytes > 65536)
            throw new ArgumentException("Plugin request must contain 40..65536 bytes", nameof(buffer));
        if (!BitConverter.IsLittleEndian) throw new InvalidOperationException("Plugin ABI requires little-endian execution");
        if (Marshal.ReadInt32(buffer, 0) != sizeBytes || Marshal.ReadInt32(buffer, 4) != 1)
            throw new ArgumentException("Plugin ABI size/version mismatch", nameof(buffer));
        Marshal.WriteInt64(buffer, 32, 0);
        Owner(unchecked((ulong)Marshal.ReadInt64(buffer, 8)));
        string ReadText(int offsetField, int lengthField)
        {
            var offset = Marshal.ReadInt32(buffer, offsetField);
            var length = Marshal.ReadInt32(buffer, lengthField);
            if (offset < 40 || offset > sizeBytes || length < 0 || length > sizeBytes - offset)
                throw new ArgumentException("Plugin text range is outside the request", nameof(buffer));
            var bytes = new byte[length];
            Marshal.Copy(buffer + offset, bytes, 0, length);
            var text = new UTF8Encoding(false, true).GetString(bytes);
            if (text.Contains('\0')) throw new ArgumentException("Plugin text contains NUL", nameof(buffer));
            return text;
        }
        var path = ReadText(16, 20);
        var typeName = ReadText(24, 28);
        if (string.IsNullOrWhiteSpace(path) || !System.IO.Path.IsPathFullyQualified(path))
            throw new ArgumentException("Plugin path must be absolute", nameof(path));
        if (typeName.Length > 4096) throw new ArgumentException("Plugin type name exceeds 4096 characters", "type_name");
        return (path, typeName);
    }
    public static int CreatePlugin(nint buffer, int sizeBytes) => Run(() =>
    {
        var (path, typeName) = Request(buffer, sizeBytes);
        var id = plugins.Value.OnOwner(() => plugins.Value.Add(path,
            PluginLoader.Create(path, typeName == "" ? null : typeName)));
        Marshal.WriteInt64(buffer, 32, id);
        return 0;
    });
    public static int ConfigureHost(nint buffer, int sizeBytes) => Run(() =>
    {
        var (path, _) = Request(buffer, sizeBytes);
        plugins.Value.Configure(path);
        return 0;
    });
    public static int PluginCommand(nint buffer, int sizeBytes) => Run(() =>
    {
        // Reserve output capacity before callbacks, so a short buffer cannot duplicate side effects.
        if (buffer == 0 || sizeBytes != 8224) throw new ArgumentException("Plugin command requires 8224 bytes", nameof(buffer));
        if (!BitConverter.IsLittleEndian || Marshal.ReadInt32(buffer, 0) != sizeBytes || Marshal.ReadInt32(buffer, 4) != 1)
            throw new ArgumentException("Plugin command ABI size/version mismatch", nameof(buffer));
        Marshal.WriteInt32(buffer, 28, 0);
        Owner(unchecked((ulong)Marshal.ReadInt64(buffer, 8)));
        var id = Marshal.ReadInt64(buffer, 16);
        string text;
        switch (Marshal.ReadInt32(buffer, 24))
        {
            case 1: text = plugins.Value.Initialize(id); break;
            case 2: plugins.Value.Deinitialize(id); text = ""; break;
            default: throw new ArgumentException("Unknown plugin lifecycle command", "command");
        }
        if (text.Length > 2048) text = text[..2048];
        var bytes = Encoding.UTF8.GetBytes(text);
        Marshal.Copy(bytes, 0, buffer + 32, bytes.Length);
        Marshal.WriteInt32(buffer, 28, bytes.Length);
        return 0;
    });
    public static int ReadError(nint buffer, int capacity)
    {
        try
        {
            lock (gate)
            {
                if (buffer == 0 || capacity < 0) return -1;
                var bytes = Encoding.UTF8.GetBytes(lastError);
                if (capacity >= bytes.Length) Marshal.Copy(bytes, 0, buffer, bytes.Length);
                return bytes.Length;
            }
        }
        catch { return -1; }
    }
}
