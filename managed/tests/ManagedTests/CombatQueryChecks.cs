using System.Runtime.CompilerServices;
using System.Buffers.Binary;
using System.Text;
using Advanced_Combat_Tracker;
using Aftercast.ActCompat;
using Aftercast.Bridge;
using Aftercast.Testing;

internal static class CombatQueryChecks
{
    private static void Check(bool condition, [CallerArgumentExpression(nameof(condition))] string? expression = null)
    {
        if (!condition) throw new Exception("Combat query check failed: " + expression);
    }

    internal static void Run() => HostLifetimeChecks.RunBounded(() =>
    {
        var directory = Directory.CreateTempSubdirectory("aftercast-combat-query-");
        try
        {
            using var host = new HostEnvironment(directory.FullName);
            var root = ActGlobals.oFormActMain!;
            CombatFixture.ConfigureCategories();
            root.ChangeZone("Zone 한글\0🙂");
            Check(host.ReadEncounter(0, true) is null);
            Check(host.ListEncounters(0, 16).Total == 0);
            void Add(int second, long amount)
            {
                root.SetEncounter(DateTime.UnixEpoch.AddSeconds(second), "Source", "Target");
                root.AddCombatAction(new(1, false, "None", new Dnum(amount), DateTime.UnixEpoch.AddSeconds(second),
                    second, "Strike 한글", "Source", "Physical", "Target"));
            }
            Add(0, 100);
            Add(2, 300);
            EncounterDetailData live = null!;
            // Drain fixture actions before asserting values; production queries deliberately do not join producers.
            host.RunWithLogReadingSuspended(() =>
            {
                live = host.ReadEncounter(0, true)!;
                Check(live.Summary.Status == EncounterStatus.Active && live.Summary.Damage == 400 && live.Combatants.Length == 2);
                Check(live.Combatants.Single(actor => actor.Name == "Source").EncDPS == 200);
            });
            var id = live.Summary.Id;
            var page = host.ListEncounters(0, 16);
            Check(page.Total == 1 && page.Items.Single().Id == id);
            Check(host.ListEncounters(1, 1).Items.Length == 0);
            Check(host.ReadEncounter(ulong.MaxValue, false) is null);
            Check(host.ReadEncounterAttacks(ulong.MaxValue, 0, 16) is null);
            root.EndCombat(true);
            Check(host.ReadEncounter(0, true) is null);
            var ended = host.ReadEncounter(id, false)!;
            Check(ended.Summary.Status == EncounterStatus.Ended && ended.Summary.Id == id && ended.Summary.Damage == 400);
            CheckEncoding(host, ended.Summary);
            var attacks = host.ReadEncounterAttacks(id, 0, 256)!;
            var strike = attacks.Items.Single(value => value.Combatant == "Source" && value.Category == "Damage" && value.Attack == "Strike 한글");
            Check(strike.Outgoing && strike.Damage == 400 && strike.Hits == 2 && strike.EncDPS == 200);
            Check(attacks.Items.Any(value => !value.Outgoing && value.Combatant == "Target"));
            Check(host.ReadEncounterAttacks(id, 1, 1)!.Items.Single() == attacks.Items[1]);
            Check(host.ReadEncounterAttacks(id, int.MaxValue, 1)!.Items.Length == 0);
            // Exercise category transitions, e.g. a page can start in All and end in an individual attack list.
            foreach (var limit in new[] { 1, 2, 3, 256 })
                for (var offset = 0; offset <= attacks.Total + 1; offset++)
                {
                    var slice = host.ReadEncounterAttacks(id, offset, limit)!;
                    Check(slice.Total == attacks.Total);
                    Check(slice.Items.SequenceEqual(attacks.Items.Skip(offset).Take(limit)));
                }
            Add(4, 7);
            root.EndCombat(true);
            Check(host.ListEncounters(0, 16).Total == 2);
            Check(host.ReadEncounter(id, false)!.Summary.Damage == 400 && live.Summary.Status == EncounterStatus.Active);
            // Removing a public ACT graph entry makes its ID absent without renumbering another encounter.
            var zone = root.ZoneList.Single(value => value.Items.Any(item => item.Items.Count != 0));
            var newer = host.ListEncounters(1, 1).Items.Single().Id;
            zone.Items.RemoveAt(zone.PopulateAll ? 1 : 0);
            Check(host.ReadEncounter(id, false) is null && host.ReadEncounter(newer, false) is not null);
            Check(host.ReadEncounterAttacks(id, 0, 1) is null);
            Check(live.Summary.Damage == 400 && strike.Damage == 400);
            CheckImports(host, root, directory.FullName);
        }
        finally { directory.Delete(true); }
        Console.WriteLine("Live combat, stable encounter IDs, owned detail and attack page checks passed.");
    });

    private static void CheckEncoding(HostEnvironment host, EncounterSummaryData expected)
    {
        var payload = CombatQueryEncoding.Encode(host, 2, 0, 0, 1);
        using var stream = new MemoryStream(payload.Array!, payload.Offset, payload.Count, writable: false);
        using var reader = new BinaryReader(stream, Encoding.UTF8);
        string Text() => new UTF8Encoding(false, true).GetString(reader.ReadBytes(reader.ReadInt32()));
        Check(reader.ReadInt32() == 1 && reader.ReadInt32() == 1);
        Check(reader.ReadUInt64() == expected.Id);
        Check(reader.ReadUInt32() == (uint)expected.Status << 1);
        Check(Text() == expected.Zone && Text() == expected.Title && Text() == expected.Character);
        Check(reader.ReadInt64() == expected.Start.Ticks && reader.ReadUInt32() == (uint)expected.Start.Kind);
        Check(reader.ReadInt64() == expected.End.Ticks && reader.ReadUInt32() == (uint)expected.End.Kind);
        Check(reader.ReadInt64() == expected.DurationTicks && reader.ReadInt64() == expected.Damage);
        Check(reader.ReadInt64() == expected.Healed && reader.ReadInt32() == expected.Combatants);
        Check(stream.Position == stream.Length);
        // A second encoding cannot rewrite a previously returned owned segment, e.g. a GUI retaining an earlier page.
        var empty = CombatQueryEncoding.Encode(null, 2, 0, 0, 1);
        Check(empty.Count == 8 && payload.Count > empty.Count);
        stream.Position = 8;
        Check(reader.ReadUInt64() == expected.Id);
        var encounter = ActGlobals.oFormActMain!.ActiveZone.ActiveEncounter!;
        var originalZone = encounter.ZoneName;
        try
        {
            // Cross encoder/buffer boundaries, e.g. a surrogate pair after 255 ASCII characters and embedded NUL.
            var zone = new string('x', 255) + "🙂" + new string('한', 4096) + "\0";
            encounter.ZoneName = zone;
            var large = CombatQueryEncoding.Encode(host, 2, 0, 0, 1).AsSpan();
            var length = BinaryPrimitives.ReadInt32LittleEndian(large[20..]);
            Check(length == Encoding.UTF8.GetByteCount(zone));
            Check(new UTF8Encoding(false, true).GetString(large.Slice(24, length)) == zone);
            encounter.ZoneName = "\uD800";
            try { CombatQueryEncoding.Encode(host, 2, 0, 0, 1); throw new Exception("Invalid UTF-16 query text was accepted"); }
            catch (EncoderFallbackException) { }
        }
        finally { encounter.ZoneName = originalZone; }
    }

    private static void CheckImports(HostEnvironment host, FormActMain root, string directory)
    {
        var path = Path.Combine(directory, "query-import.log");
        root.GetDateTimeFromLog = _ => DateTime.UnixEpoch;
        root.OnLogLineRead += (import, _) =>
        {
            if (!import) return;
            root.SetEncounter(DateTime.UnixEpoch, "Source", "Target");
            root.AddCombatAction(new(1, false, "None", new Dnum(25), DateTime.UnixEpoch,
                99, "Import", "Source", "Physical", "Target"));
        };
        var observed = 0;
        root.OnCombatEnd += (import, _) =>
        {
            if (!import) return;
            var importing = host.ListEncounters(0, 256).Items.Where(item => item.Status == EncounterStatus.Importing).ToArray();
            Check(importing.Length == 1 && importing[0].IsImport);
            Check(host.ReadEncounter(importing[0].Id, false)!.Summary.Status == EncounterStatus.Importing);
            Check(host.ReadEncounter(0, true) is null);
            observed++;
        };
        File.WriteAllText(path, "valid\n");
        host.ImportLog(path);
        var first = host.ListEncounters(0, 256).Items.Single(item => item.IsImport);
        Check(first.Status == EncounterStatus.Ended);
        // A later failed import must mark only its new encounter; earlier successful history stays complete.
        File.WriteAllBytes(path, [(byte)'v', (byte)'\n', 255]);
        try { host.ImportLog(path); throw new Exception("Invalid UTF-8 import succeeded"); }
        catch (InvalidDataException) { }
        Check(observed == 2);
        var imported = host.ListEncounters(0, 256).Items.Where(item => item.IsImport).ToArray();
        Check(imported.Length == 2 && imported[0].Id == first.Id);
        Check(imported[0].Status == EncounterStatus.Ended && imported[1].Status == EncounterStatus.PartialImport);
        Check(host.ReadEncounter(imported[1].Id, false)!.Summary.Damage == 25);
    }
}
