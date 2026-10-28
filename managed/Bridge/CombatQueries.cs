using System.Runtime.InteropServices;
using System.Text;
using Aftercast.ActCompat;

namespace Aftercast.Bridge;

public static partial class EntryPoint
{
    public static int QueryCombat(nint buffer, int sizeBytes) => Run(() =>
    {
        const int header = 48;
        if (buffer == 0 || sizeBytes < header || sizeBytes > header + CombatQueryEncoding.MaximumBytes)
            throw new ArgumentException("Combat query capacity is outside supported bounds");
        ValidateHeader(buffer, sizeBytes, "Combat query ABI size/version mismatch");
        Owner(unchecked((ulong)Marshal.ReadInt64(buffer, 8)));
        var mode = Marshal.ReadInt32(buffer, 16);
        var offset = Marshal.ReadInt32(buffer, 20);
        var id = unchecked((ulong)Marshal.ReadInt64(buffer, 24));
        var limit = Marshal.ReadInt32(buffer, 32);
        if (mode is < 1 or > 10 || offset < 0 || limit is < 1 or > 256 ||
            (mode is 1 or 2 ? id != 0 : id == 0) || (mode is 1 or 3 or 7 or 10 && offset != 0))
            throw new ArgumentException("Invalid combat query mode, ID or page");
        for (var field = 36; field < header; field += 4)
            if (Marshal.ReadInt32(buffer, field) != 0) throw new ArgumentException("Combat query output must initially be zero");
        string? incomingCombatant = null;
        if (mode == 10)
        {
            if (sizeBytes < header + 4) throw new ArgumentException("Missing incoming combatant");
            var length = Marshal.ReadInt32(buffer, header);
            if (length is < 0 or > 4096 || length > sizeBytes - header - 4)
                throw new ArgumentException("Invalid incoming combatant length");
            var text = new byte[length];
            Marshal.Copy(buffer + header + 4, text, 0, length);
            incomingCombatant = new UTF8Encoding(false, true).GetString(text);
        }
        // Capture changes the active snapshot; reserve its fixed response before admission.
        if (mode is 7 or 10 && sizeBytes < header + 16)
        {
            Marshal.WriteInt32(buffer, 40, header + 16);
            return 1;
        }
        var bytes = plugins.Value.QueryCombat(mode, id, offset, limit, incomingCombatant);
        if (bytes.Count > sizeBytes - header)
        {
            Marshal.WriteInt32(buffer, 40, checked(header + bytes.Count));
            return 1;
        }
        Marshal.Copy(bytes.Array!, bytes.Offset, buffer + header, bytes.Count);
        Marshal.WriteInt32(buffer, 36, bytes.Count);
        return 0;
    });
}

internal static class CombatQueryEncoding
{
    internal const int MaximumBytes = 64 * 1024 * 1024;
    private static readonly UTF8Encoding Utf8 = new(false, true);

    internal static ArraySegment<byte> Encode(HostEnvironment? host, int mode, ulong id, int offset, int limit, string? incomingCombatant = null)
    {
        using var stream = new MemoryStream();
        using var writer = new BinaryWriter(stream, Utf8, true);
        void Budget(long length)
        {
            if (length < 0 || stream.Length + length > MaximumBytes)
                throw new InvalidDataException("Combat query exceeds 64 MiB");
        }
        void Text(string text)
        {
            var length = Utf8.GetByteCount(text);
            Budget(4L + length);
            writer.Write(length);
            // The char-span overload writes UTF-8 without another length prefix or a per-string byte array.
            writer.Write(text.AsSpan());
        }
        void OptionalText(string? value) { if(value is null){Budget(4);writer.Write(-1);}else Text(value); }
        void Time(DateTime time) { writer.Write(time.Ticks); writer.Write((uint)time.Kind); }
        void Summary(EncounterSummaryData value)
        {
            Budget(12);
            writer.Write(value.Id);
            writer.Write((value.IsImport ? 1u : 0) | ((uint)value.Status << 1));
            Text(value.Zone); Text(value.Title); Text(value.Character);
            Budget(52);
            Time(value.Start); Time(value.End);
            writer.Write(value.DurationTicks); writer.Write(value.Damage); writer.Write(value.Healed);
            writer.Write(value.Combatants);
        }
        if (mode == 2)
        {
            var page = host?.ListEncounters(offset, limit) ?? new(0, []);
            writer.Write(page.Total); writer.Write(page.Items.Length);
            foreach (var value in page.Items) Summary(value);
        }
        else if (mode == 4)
        {
            var page = host?.ReadEncounterAttacks(id, offset, limit);
            writer.Write(page is null ? 0u : 1u);
            if (page is not null)
            {
                writer.Write(page.Total); writer.Write(page.Items.Length);
                foreach (var value in page.Items)
                {
                    Text(value.Combatant); Text(value.Category); Text(value.Attack);
                    Budget(56);
                    writer.Write(value.Outgoing ? 1u : 0u);
                    writer.Write(value.Damage); writer.Write(value.DurationTicks);
                    writer.Write(value.DPS); writer.Write(value.EncDPS);
                    writer.Write(value.Swings); writer.Write(value.Hits); writer.Write(value.CritHits);
                    writer.Write(value.Misses); writer.Write(value.Blocked);
                }
            }
        }
        else if (mode == 5)
        {
            var page = host?.ReadEncounterActions(id, offset, limit);
            writer.Write(page is null ? 0u : 1u);
            if (page is not null)
            {
                writer.Write(page.Total); writer.Write(page.Items.Length);
                foreach (var action in page.Items)
                {
                    Budget(16); Time(action.Time); writer.Write(action.TimeSorter);
                    Text(action.Attacker); Text(action.Victim); Text(action.Attack); Text(action.Damage);
                    Budget(20); writer.Write(action.SourceIndex);writer.Write(action.Amount);writer.Write(action.SwingType);
                    writer.Write(action.Critical?1u:0u);
                    OptionalText(action.Special);OptionalText(action.DamageType);OptionalText(action.RawLabel);OptionalText(action.RawSuffix);
                }
            }
        }
        else if (mode is 7 or 10)
        {
            var value = mode == 10 ? host?.CaptureIncomingActionSnapshot(id, incomingCombatant!) : host?.CaptureActionSnapshot(id);
            writer.Write(value is null ? 0u : 1u);
            if (value is not null) { writer.Write(value.Revision); writer.Write(value.Total); }
        }
        else if (mode == 8)
        {
            var page = host?.ReadActionSnapshot(id, offset, limit)
                ?? throw new InvalidOperationException("Action snapshot is unavailable");
            writer.Write(page.Total); writer.Write(page.Items.Length);
            foreach (var item in page.Items)
            {
                var action = item.Action;
                Budget(16); Time(action.Time); writer.Write(action.TimeSorter);
                Text(action.Attacker); Text(action.Victim); Text(action.Attack); Text(action.Damage);
                Budget(20); writer.Write(action.SourceIndex); writer.Write(action.Amount); writer.Write(action.SwingType);
                writer.Write(action.Critical ? 1u : 0u);
                OptionalText(action.Special); OptionalText(action.DamageType); OptionalText(action.RawLabel); OptionalText(action.RawSuffix);
                Budget(4); writer.Write((item.RegisteredDamage ? 1u : 0u) | (item.RegisteredHealing ? 2u : 0u) | (item.IsDeath ? 4u : 0u));
                Text(item.NumericTooltip);
            }
        }
        else if (mode is 6 or 9)
        {
            var value = mode == 9 ? host?.ReadActionSnapshotLogs(id, offset, limit) : host?.ReadEncounterLogs(id, offset, limit);
            if (mode == 9 && value is null) throw new InvalidOperationException("Action snapshot is unavailable");
            writer.Write(value is null ? 0u : 1u);
            if (value is not null)
            {
                writer.Write(value.Dropped); writer.Write(value.Page.Total); writer.Write(value.Page.Items.Length);
                foreach (var line in value.Page.Items)
                {
                    Budget(24); Time(line.Time); writer.Write(line.TimeSorter); writer.Write(line.Type);
                    writer.Write(line.SourceIndex);
                    Text(line.Text);
                }
            }
        }
        else
        {
            var value = host?.ReadEncounter(id, mode == 1);
            writer.Write(value is null ? 0u : 1u);
            if (value is not null)
            {
                Summary(value.Summary);
                // Reuse the value encoder without attaching another encounter's published revision/status.
                var summary = value.Summary;
                var snapshot = CombatSnapshotEncoding.Encode(new(0, summary.IsImport, CombatSnapshotStatus.Ended,
                    summary.Zone, summary.Title, summary.Character, summary.Start, summary.End,
                    summary.DurationTicks, summary.Damage, summary.Healed, value.Combatants), MaximumBytes);
                Budget(4L + snapshot.Bytes.Length);
                writer.Write(snapshot.Bytes.Length);
                writer.Write(snapshot.Bytes);
            }
        }
        Budget(0);
        // The segment retains the stream's managed array after disposal without copying unused capacity.
        return new(stream.GetBuffer(), 0, checked((int)stream.Length));
    }
}
