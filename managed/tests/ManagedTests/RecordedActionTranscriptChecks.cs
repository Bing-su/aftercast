using System.Text.Json;
using Bxivact.TestContracts;

internal static class RecordedActionTranscriptChecks
{
    internal static void Run()
    {
        var source = Fixture();
        source.Validate();
        Invalid(source with { SchemaVersion = 2 });
        Invalid(source with { Provenance = source.Provenance with { InputSha256 = "not-a-hash" } });
        Invalid(source with { Actions = new TranscriptAction[RecordedActionTranscript.MaximumActions + 1] });
        Invalid(source with { Actions = [source.Actions[0] with { EncounterId = 99 }] });
        Invalid(source with { Actions = [source.Actions[0] with { TimeKind = 3 }] });
        Invalid(source with { Actions = [source.Actions[0] with { TimeTicks = -1 }] });
        Invalid(source with { Actions = [source.Actions[0] with { Attacker = "\ud800" }] });
        Invalid(source with { Configuration = source.Configuration with { Outgoing = [new("Damage", "\udfff", 0, 0)] } });
        Invalid(source with { Encounters = [source.Encounters[0] with { SelectiveParsing = true }] });
        Invalid(source with { Encounters = [source.Encounters[0] with { Allies = ["Missing"] }] });
        Invalid(source with { Configuration = source.Configuration with { CategoryNames = [] } });
        Invalid(source with { Configuration = source.Configuration with { OutgoingLinks = [new(1, ["Missing"])] } });
        Invalid(source with { Configuration = source.Configuration with { OutgoingLinks = [new(3, ["Heal"]), new(1, ["Damage"])] } });
        var directory = Directory.CreateTempSubdirectory("bxivact-recorded-schema-");
        try
        {
            var path = Path.Combine(directory.FullName, "transcript.json");
            source.Write(path);
            var restored = RecordedActionTranscript.Load(path);
            Check(restored.Configuration.Outgoing.SequenceEqual(source.Configuration.Outgoing));
            Check(restored.Configuration.Incoming.SequenceEqual(source.Configuration.Incoming));
            Check(restored.Configuration.OutgoingLinks[0].Categories.SequenceEqual(new[] { "Damage", "Damage" }));
            Check(restored.Actions.SequenceEqual(source.Actions));
            Check(restored.Encounters[0].Actors.SequenceEqual(source.Encounters[0].Actors));
            Check(restored.Configuration.Outgoing[^1].Key == "All outgoing");
            // Missing required constructor fields and unexpected JSON fields must fail instead of inheriting defaults.
            File.WriteAllText(path, "{}");
            Throws<JsonException>(() => RecordedActionTranscript.Load(path));
            source.Write(path);
            var text = File.ReadAllText(path);
            File.WriteAllText(path, text[..^1] + ",\"Unexpected\":0}");
            Throws<JsonException>(() => RecordedActionTranscript.Load(path));
            using (var oversized = File.Create(path)) oversized.SetLength(RecordedActionTranscript.MaximumBytes + 1);
            Throws<InvalidDataException>(() => RecordedActionTranscript.Load(path));
        }
        finally { directory.Delete(true); }
    }

    private static RecordedActionTranscript Fixture()
    {
        var hash = new string('a', 64);
        return new(1, new(hash, hash, hash, hash), new("en-US", "en-US",
            [new("Damage", "Damage", -1, 0), new("Heal", "Heal", 1, 0), new("All outgoing", "All", 0, 0)],
            [new("Taken", "Taken", -1, 0), new("Received", "Received", 1, 0), new("All incoming", "All", 0, 0)],
            [new(1, ["Damage", "Damage"]), new(3, ["Heal"])], [new(1, ["Taken"]), new(3, ["Received"])],
            ["Auto", "Damage", "Heal", "Taken", "Received", "Power", "Drain", "Cure"], [1], [3], false, false, false),
            [new(1, "Player", "Zone", false, false, false,
                [new("Player", 1, 0, 0, 0), new("Enemy", 0, 0, 1, 0)], ["Player"], 1, 0)],
            [new(1, 1, false, "None", 1, "1", DateTime.UnixEpoch.Ticks, 1, 7, "Strike", "Player", "Physical", "Enemy"),
             new(1, 1, false, "None", -1, "Miss", DateTime.UnixEpoch.Ticks, 1, 8, "Strike", "Player", "Physical", "Enemy")]);
    }

    private static void Invalid(RecordedActionTranscript value) => Throws<InvalidDataException>(value.Validate);
    private static void Throws<T>(Action action) where T : Exception
    {
        try { action(); } catch (T) { return; }
        throw new Exception("Recorded-action schema accepted invalid input: " + typeof(T).Name);
    }
    private static void Check(bool condition)
    {
        if (!condition) throw new Exception("Recorded-action schema changed serialized observation order");
    }
}
