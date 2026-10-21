using System.Runtime.InteropServices;
using System.Text;
using Aftercast.ActCompat;

namespace Aftercast.Bridge;

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
        if (command is not (2 or 3 or 4 or 5)) return 6;
        if (active is null) return 4;
        if (active.Id != session) return 5;
        if (command == 2) return 0;
        if (command == 4)
        {
            Marshal.WriteInt32(buffer, 12, plugins.Value.Pump());
            return 0;
        }
        if (command == 5)
        {
            // End the owned encounter without stopping the session or creating an unconfigured host.
            plugins.Value.EndCombat();
            return 0;
        }
        // A rejected reentrant wait leaves the session active so its owner can stop it after dispatch returns.
        plugins.Value.EnsureCanStop();
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
            plugins.Value.Construct(path, typeName == "" ? null : typeName)));
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

    public static int ImportLog(nint buffer, int sizeBytes) => Run(() =>
    {
        const int headerSize = 64;
        // Validate input and reserve the entire result before invoking any plugin or touching history.
        if (buffer == 0 || sizeBytes < headerSize || sizeBytes > 65536)
            throw new ArgumentException("Import request must contain 64..65536 bytes", nameof(buffer));
        if (!BitConverter.IsLittleEndian || Marshal.ReadInt32(buffer, 0) != sizeBytes || Marshal.ReadInt32(buffer, 4) != 1)
            throw new ArgumentException("Import ABI size/version mismatch", nameof(buffer));
        Owner(unchecked((ulong)Marshal.ReadInt64(buffer, 8)));
        var offset = Marshal.ReadInt32(buffer, 16);
        var length = Marshal.ReadInt32(buffer, 20);
        if (offset < headerSize || offset > sizeBytes || length <= 0 || length > sizeBytes - offset)
            throw new ArgumentException("Import path is outside the request", nameof(buffer));
        var bytes = new byte[length];
        Marshal.Copy(buffer + offset, bytes, 0, length);
        var path = new UTF8Encoding(false, true).GetString(bytes);
        if (path.Contains('\0') || !Path.IsPathFullyQualified(path))
            throw new ArgumentException("Import path must be absolute and contain no NUL", nameof(buffer));
        var result = plugins.Value.ImportLog(path);
        Marshal.WriteInt64(buffer, 24, result.LinesRead);
        Marshal.WriteInt64(buffer, 32, result.RecordsDispatched);
        Marshal.WriteInt32(buffer, 40, result.Encounters);
        Marshal.WriteInt32(buffer, 44, result.Combatants);
        Marshal.WriteInt64(buffer, 48, result.Damage);
        Marshal.WriteInt64(buffer, 56, result.Healed);
        return 0;
    });

    public static int LogCommand(nint buffer, int sizeBytes) => Run(() =>
    {
        const int headerSize = 64;
        const int maximumSize = headerSize + 64 * 1024 * 1024;
        if (buffer == 0 || sizeBytes < headerSize || sizeBytes > maximumSize)
            throw new ArgumentException("Log command capacity is outside supported bounds", nameof(buffer));
        if (!BitConverter.IsLittleEndian || Marshal.ReadInt32(buffer, 0) != sizeBytes || Marshal.ReadInt32(buffer, 4) != 1)
            throw new ArgumentException("Log command ABI size/version mismatch", nameof(buffer));
        Owner(unchecked((ulong)Marshal.ReadInt64(buffer, 8)));
        var id = unchecked((ulong)Marshal.ReadInt64(buffer, 16));
        var command = Marshal.ReadInt32(buffer, 24);
        if ((command is 1 or 3) && sizeBytes != headerSize)
            throw new ArgumentException("Subscription commands require a 64-byte header", nameof(buffer));
        switch (command)
        {
            case 1:
                var subscription = plugins.Value.SubscribeLogs(Marshal.ReadInt32(buffer, 28), Marshal.ReadInt32(buffer, 56));
                Marshal.WriteInt64(buffer, 16, unchecked((long)subscription));
                return 0;
            case 3:
                plugins.Value.UnsubscribeLogs(id);
                return 0;
            case 2:
                var result = plugins.Value.DrainLogs(id, sizeBytes - headerSize);
                // Insufficient capacity does not consume either pending records or their separate loss state.
                var required = result.RequiredPayloadCapacity == 0 ? 0 : headerSize + result.RequiredPayloadCapacity;
                Marshal.WriteInt32(buffer, 28, result.Records);
                Marshal.WriteInt64(buffer, 32, unchecked((long)result.FirstDropped));
                Marshal.WriteInt64(buffer, 40, unchecked((long)result.LastDropped));
                Marshal.WriteInt64(buffer, 48, unchecked((long)result.Dropped));
                Marshal.WriteInt32(buffer, 56, result.Payload.Length);
                Marshal.WriteInt32(buffer, 60, required);
                Marshal.Copy(result.Payload, 0, buffer + headerSize, result.Payload.Length);
                return required == 0 ? 0 : 1;
            default: throw new ArgumentException("Unknown log subscription command", nameof(command));
        }
    });

    public static int SpeechCommand(nint buffer, int sizeBytes) => Run(() =>
    {
        const int headerSize = 32;
        // A drain always reserves the maximum text size before dequeue; retries cannot lose or duplicate an utterance.
        if (buffer == 0 || !BitConverter.IsLittleEndian || sizeBytes is not (headerSize or headerSize + 4096) ||
            Marshal.ReadInt32(buffer, 0) != sizeBytes || Marshal.ReadInt32(buffer, 4) != 1 ||
            Marshal.ReadInt64(buffer, 24) != 0)
            throw new ArgumentException("Invalid speech command header", nameof(buffer));
        Owner(unchecked((ulong)Marshal.ReadInt64(buffer, 8)));
        var command = Marshal.ReadInt32(buffer, 16);
        var argument = Marshal.ReadInt32(buffer, 20);
        if (command == 1 && sizeBytes == headerSize && argument is 0 or 1)
            plugins.Value.ConfigureSpeech(argument == 1);
        else if (command == 2 && sizeBytes == headerSize + 4096 && argument == 0)
        {
            var text = plugins.Value.DrainSpeech();
            Marshal.Copy(text, 0, buffer + headerSize, text.Length);
            Marshal.WriteInt32(buffer, 20, text.Length);
        }
        else throw new ArgumentException("Invalid speech command or capacity", nameof(buffer));
        return 0;
    });

    public static int ReadCombatSnapshot(nint buffer, int sizeBytes) => Run(() =>
    {
        const int headerSize = 64;
        const int maximumSize = headerSize + 64 * 1024 * 1024;
        // Validate all input before querying the graph; a capacity retry never consumes a version.
        if (buffer == 0 || sizeBytes < headerSize || sizeBytes > maximumSize)
            throw new ArgumentException("Snapshot capacity is outside supported bounds", nameof(buffer));
        if (!BitConverter.IsLittleEndian || Marshal.ReadInt32(buffer, 0) != sizeBytes || Marshal.ReadInt32(buffer, 4) != 1)
            throw new ArgumentException("Snapshot ABI size/version mismatch", nameof(buffer));
        Owner(unchecked((ulong)Marshal.ReadInt64(buffer, 8)));
        for (var offset = 24; offset < headerSize; offset += 8)
            if (Marshal.ReadInt64(buffer, offset) != 0)
                throw new ArgumentException("Snapshot output fields must initially be zero", nameof(buffer));
        var after = unchecked((ulong)Marshal.ReadInt64(buffer, 16));
        var snapshot = plugins.Value.ReadCombatSnapshot(after);
        if (snapshot is null) return 0;
        var result = CombatSnapshotEncoding.Encode(snapshot, sizeBytes - headerSize);
        if (result.RequiredPayloadCapacity != 0)
        {
            Marshal.WriteInt32(buffer, 44, headerSize + result.RequiredPayloadCapacity);
            return 1;
        }
        Marshal.Copy(result.Bytes, 0, buffer + headerSize, result.Bytes.Length);
        Marshal.WriteInt64(buffer, 24, unchecked((long)snapshot.Revision));
        Marshal.WriteInt32(buffer, 32, snapshot.Combatants.Length);
        Marshal.WriteInt32(buffer, 36, 1 | (snapshot.IsImport ? 2 : 0) | ((int)snapshot.Status << 2));
        Marshal.WriteInt32(buffer, 40, result.Bytes.Length);
        Marshal.WriteInt64(buffer, 48, unchecked((long)(after == 0 ? 0 : snapshot.Revision - after - 1)));
        return 0;
    });

    public static int ReadDiagnostics(nint buffer, int sizeBytes) => Run(() =>
    {
        const int headerSize = 32;
        const int bufferSize = headerSize + 1024 * 1024;
        if (buffer == 0 || sizeBytes != bufferSize)
            throw new ArgumentException($"Diagnostics request requires {bufferSize} bytes", nameof(buffer));
        if (!BitConverter.IsLittleEndian || Marshal.ReadInt32(buffer, 0) != bufferSize || Marshal.ReadInt32(buffer, 4) != 1)
            throw new ArgumentException("Diagnostics ABI size/version mismatch", nameof(buffer));
        Owner(unchecked((ulong)Marshal.ReadInt64(buffer, 8)));
        // Capture count and retained messages together so workers cannot produce a mismatched snapshot.
        var (total, messages) = plugins.Value.DiagnosticsSnapshot;
        var encoded = new byte[messages.Length][];
        var used = 0;
        for (var index = 0; index < messages.Length; index++)
        {
            encoded[index] = Encoding.UTF8.GetBytes(messages[index]);
            used = checked(used + sizeof(int) + encoded[index].Length);
        }
        // Validate capacity before writes; diagnostics are snapshots and never consume retained entries.
        if (messages.Length > 32 || used > bufferSize - headerSize)
            throw new InvalidOperationException("Retained diagnostics exceed ABI capacity");
        var offset = headerSize;
        foreach (var bytes in encoded)
        {
            Marshal.WriteInt32(buffer, offset, bytes.Length);
            offset += sizeof(int);
            Marshal.Copy(bytes, 0, buffer + offset, bytes.Length);
            offset += bytes.Length;
        }
        Marshal.WriteInt64(buffer, 16, total);
        Marshal.WriteInt32(buffer, 24, messages.Length);
        Marshal.WriteInt32(buffer, 28, used);
        return 0;
    });
}
