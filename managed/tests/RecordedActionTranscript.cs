using System.Globalization;
using System.Diagnostics.CodeAnalysis;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace Bxivact.TestContracts;

public sealed record TranscriptProvenance(string InputSha256, string MainPluginSha256, string ParseSha256, string OriginalActSha256);
public sealed record TranscriptCategory(string Key, string Label, int AllyValue, int Argb);
public sealed record TranscriptLink(int SwingType, string[] Categories);
public sealed record TranscriptConfiguration(string Culture, string UICulture, TranscriptCategory[] Outgoing,
    TranscriptCategory[] Incoming, TranscriptLink[] OutgoingLinks, TranscriptLink[] IncomingLinks,
    string[] CategoryNames, int[] DamageSwingTypes, int[] HealingSwingTypes,
    bool RestrictToAll, bool LongDuration, bool BlockIsHit);
public sealed record TranscriptActor(string Name, long Damage, long Healed, long DamageTaken, long HealsTaken);
public sealed record TranscriptEncounter(int Id, string Character, string Zone, bool DuplicateDetection,
    bool SelectiveParsing, bool IgnoreEnemies, TranscriptActor[] Actors, string[] Allies, long Damage, long Healed);
public sealed record TranscriptAction(int EncounterId, int SwingType, bool Critical, string Special,
    long Damage, string DamageString, long TimeTicks, int TimeKind, int TimeSorter,
    string AttackType, string Attacker, string DamageType, string Victim);

// Keep this test-only schema independent of either ACT implementation; arrays preserve observed registration/action order.
public sealed record RecordedActionTranscript(int SchemaVersion, TranscriptProvenance Provenance,
    TranscriptConfiguration Configuration, TranscriptEncounter[] Encounters, TranscriptAction[] Actions)
{
    public const long MaximumBytes = 128 * 1024 * 1024;
    public const int MaximumActions = 250000;
    private static readonly UTF8Encoding StrictText = new(false, true);
    private static readonly JsonSerializerOptions Options = new()
    {
        MaxDepth = 32,
        RespectRequiredConstructorParameters = true,
        UnmappedMemberHandling = JsonUnmappedMemberHandling.Disallow
    };

    public static RecordedActionTranscript Load(string path)
    {
        using var input = File.OpenRead(path);
        Require(input.Length is > 0 and <= MaximumBytes, "transcript byte limit");
        // Read a fixed bounded image, rather than letting a growing file bypass the size admission check.
        var bytes = new byte[checked((int)input.Length)];
        input.ReadExactly(bytes);
        Require(input.ReadByte() == -1, "transcript changed during read");
        var result = JsonSerializer.Deserialize<RecordedActionTranscript>(bytes, Options)
            ?? throw new InvalidDataException("Null recorded-action transcript");
        result.Validate();
        return result;
    }

    public void Write(string path)
    {
        Validate();
        var bytes = JsonSerializer.SerializeToUtf8Bytes(this, Options);
        Require(bytes.LongLength <= MaximumBytes, "transcript byte limit");
        File.WriteAllBytes(path, bytes);
    }

    public void Validate()
    {
        Require(SchemaVersion == 1 && Provenance is not null && Configuration is not null, "schema/version");
        foreach (var hash in new[] { Provenance.InputSha256, Provenance.MainPluginSha256, Provenance.ParseSha256, Provenance.OriginalActSha256 })
            Require(hash is { Length: 64 } && hash.All(character => character is >= '0' and <= '9' or >= 'a' and <= 'f'), "SHA-256");
        var config = Configuration;
        Text(config.Culture); Text(config.UICulture);
        _ = CultureInfo.GetCultureInfo(config.Culture);
        _ = CultureInfo.GetCultureInfo(config.UICulture);
        var outgoing = Categories(config.Outgoing);
        var incoming = Categories(config.Incoming);
        Require(!outgoing.Overlaps(incoming), "overlapping category keys");
        Links(config.OutgoingLinks, outgoing); Links(config.IncomingLinks, incoming);
        Require(config.CategoryNames is { Length: 8 }, "eight host category names");
        foreach (var name in config.CategoryNames) Text(name);
        Require(outgoing.Contains(config.CategoryNames[1]) && outgoing.Contains(config.CategoryNames[2]) &&
            incoming.Contains(config.CategoryNames[3]) && incoming.Contains(config.CategoryNames[4]), "numeric getter category names");
        Require(config.DamageSwingTypes is { Length: <= 4096 } && config.HealingSwingTypes is { Length: <= 4096 }, "swing type count");
        Require(Encounters is { Length: > 0 and <= 4096 } && Actions is { Length: > 0 and <= MaximumActions }, "encounter/action count");
        var ids = new HashSet<int>();
        var actorCount = 0;
        foreach (var encounter in Encounters)
        {
            Require(encounter is not null && encounter.Id >= 0 && ids.Add(encounter.Id), "encounter identity");
            // Selection requires a real ACT UI policy and is outside this numeric replay slice.
            Require(!encounter.SelectiveParsing && !encounter.IgnoreEnemies, "unsupported selective parsing");
            Text(encounter.Character); Text(encounter.Zone);
            Require(encounter.Actors is { Length: > 0 and <= 65536 } && encounter.Allies is { Length: <= 65536 }, "actor/allies count");
            actorCount = checked(actorCount + encounter.Actors.Length);
            Require(actorCount <= 65536, "total actor count");
            var names = new HashSet<string>(StringComparer.Ordinal);
            foreach (var actor in encounter.Actors)
            {
                Require(actor is not null, "null actor");
                Text(actor.Name);
                Require(names.Add(actor.Name), "duplicate expected actor");
            }
            var allies = new HashSet<string>(StringComparer.Ordinal);
            foreach (var name in encounter.Allies)
            {
                Text(name);
                Require(names.Contains(name) && allies.Add(name), "ally identity");
            }
        }
        foreach (var action in Actions)
        {
            Require(action is not null && ids.Contains(action.EncounterId), "action encounter identity");
            Require(action.TimeTicks >= DateTime.MinValue.Ticks && action.TimeTicks <= DateTime.MaxValue.Ticks && action.TimeKind is >= 0 and <= 2, "action time");
            Text(action.Special); Text(action.DamageString); Text(action.AttackType);
            Text(action.Attacker); Text(action.DamageType); Text(action.Victim);
        }
    }

    private static HashSet<string> Categories(TranscriptCategory[] categories)
    {
        Require(categories is { Length: > 0 and <= 4096 }, "category count");
        var names = new HashSet<string>(StringComparer.Ordinal);
        foreach (var category in categories)
        {
            Require(category is not null, "null category");
            Text(category.Key); Text(category.Label);
            Require(names.Add(category.Key), "duplicate category key");
        }
        return names;
    }

    private static void Links(TranscriptLink[] links, HashSet<string> categories)
    {
        Require(links is { Length: <= 4096 }, "link count");
        int? previous = null;
        foreach (var link in links)
        {
            Require(link is not null && (previous is null || link.SwingType > previous), "sorted unique swing links");
            previous = link.SwingType;
            Require(link.Categories is { Length: <= 4096 }, "category link count");
            foreach (var name in link.Categories)
            {
                Text(name);
                Require(categories.Contains(name), "unregistered category link");
            }
        }
    }

    private static void Text(string value)
    {
        Require(value is { Length: <= 65536 }, "text length/null");
        // JSON serializers can replace malformed UTF-16; reject it before changing an observed action's text.
        try { _ = StrictText.GetByteCount(value); }
        catch (EncoderFallbackException error)
        {
            throw new InvalidDataException("Invalid recorded-action transcript: malformed UTF-16", error);
        }
    }
    private static void Require([DoesNotReturnIf(false)] bool condition, string detail)
    {
        if (!condition) throw new InvalidDataException("Invalid recorded-action transcript: " + detail);
    }
}
