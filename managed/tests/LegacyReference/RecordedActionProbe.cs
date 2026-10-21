using System.Drawing;
using System.Globalization;
using Advanced_Combat_Tracker;
using Aftercast.TestContracts;

namespace Aftercast.LegacyReference;

// Replay committed parser output through genuine public ACT APIs; expected values never drive action routing or allies.
public static class RecordedActionProbe
{
    public static string Run(string path)
    {
        var transcript = RecordedActionTranscript.Load(path);
        Check(typeof(EncounterData).Assembly.FullName ==
            "Advanced Combat Tracker, Version=3.8.5.288, Culture=neutral, PublicKeyToken=a946b61e93d97868" &&
            transcript.Provenance.OriginalActSha256 == "934fc00d0a3c8325b0f7445358860b4c013c5dab3d8fff012512acf6502dd657",
            "ACT contract identity and original artifact provenance");
        Check(ActGlobals.oFormActMain is null, "isolated process without main form");
        var saved = SaveConfiguration();
        var culture = CultureInfo.CurrentCulture;
        var uiCulture = CultureInfo.CurrentUICulture;
        var character = ActGlobals.charName;
        var registries = (CombatantData.OutgoingDamageTypeDataObjects, CombatantData.IncomingDamageTypeDataObjects,
            CombatantData.SwingTypeToDamageTypeDataLinksOutgoing, CombatantData.SwingTypeToDamageTypeDataLinksIncoming,
            CombatantData.DamageSwingTypes, CombatantData.HealingSwingTypes);
        try
        {
            Install(transcript.Configuration);
            var encounters = new Dictionary<int, EncounterData>();
            foreach (var header in transcript.Encounters)
            {
                ActGlobals.charName = header.Character;
                var zone = new ZoneData(DateTime.MinValue, header.Zone, false, false, false);
                var encounter = new EncounterData(header.Character, header.Zone, zone)
                    { DuplicateDetection = header.DuplicateDetection };
                zone.Items.Add(encounter);
                zone.ActiveEncounter = encounter;
                encounters.Add(header.Id, encounter);
            }
            foreach (var entry in transcript.Actions)
            {
                var encounter = encounters[entry.EncounterId];
                ActGlobals.charName = encounter.CharName;
                var action = new MasterSwing(entry.SwingType, entry.Critical, entry.Special,
                    new Dnum(entry.Damage, entry.DamageString), new DateTime(entry.TimeTicks, (DateTimeKind)entry.TimeKind),
                    entry.TimeSorter, entry.AttackType, entry.Attacker, entry.DamageType, entry.Victim);
                encounter.Parent.AddCombatAction(action);
            }
            foreach (var expected in transcript.Encounters) Compare(encounters[expected.Id], expected);
            // Counts summarize the bounded genuine replay; duration, titles and parser acceptance are separate contracts.
            return $"Recorded actions: {transcript.Actions.Length}; encounters: {transcript.Encounters.Length}; " +
                $"actors: {transcript.Encounters.Sum(encounter => encounter.Actors.Length)}";
        }
        finally
        {
            Install(saved);
            // Restore the caller's actual registry/list identities as well as scalar configuration.
            CombatantData.OutgoingDamageTypeDataObjects = registries.OutgoingDamageTypeDataObjects;
            CombatantData.IncomingDamageTypeDataObjects = registries.IncomingDamageTypeDataObjects;
            CombatantData.SwingTypeToDamageTypeDataLinksOutgoing = registries.SwingTypeToDamageTypeDataLinksOutgoing;
            CombatantData.SwingTypeToDamageTypeDataLinksIncoming = registries.SwingTypeToDamageTypeDataLinksIncoming;
            CombatantData.DamageSwingTypes = registries.DamageSwingTypes;
            CombatantData.HealingSwingTypes = registries.HealingSwingTypes;
            ActGlobals.charName = character;
            CultureInfo.CurrentCulture = culture;
            CultureInfo.CurrentUICulture = uiCulture;
        }
    }

    private static void Compare(EncounterData encounter, TranscriptEncounter expected)
    {
        Check(encounter.NumCombatants == expected.Actors.Length, $"encounter {expected.Id} actor count");
        var seen = new HashSet<CombatantData>(ReferenceEqualityComparer.Instance);
        foreach (var values in expected.Actors)
        {
            var actor = encounter.GetCombatant(values.Name);
            Check(actor is not null && seen.Add(actor) && actor.Name == values.Name, $"encounter {expected.Id} actor identity");
            Check(actor.Damage == values.Damage && actor.Healed == values.Healed &&
                actor.DamageTaken == values.DamageTaken && actor.HealsTaken == values.HealsTaken,
                $"encounter {expected.Id} actor numeric totals: {values.Name}");
        }
        var allies = AutomaticAlliesProbe.ReadAllies(encounter).Select(actor => actor.Name).ToArray();
        Check(allies.Length == expected.Allies.Length && new HashSet<string>(allies, StringComparer.Ordinal).SetEquals(expected.Allies),
            $"encounter {expected.Id} automatic allies");
        Check(encounter.Damage == expected.Damage && encounter.Healed == expected.Healed,
            $"encounter {expected.Id} allied totals");
    }

    private static TranscriptConfiguration SaveConfiguration() => new(CultureInfo.CurrentCulture.Name,
        CultureInfo.CurrentUICulture.Name,
        CombatantData.OutgoingDamageTypeDataObjects.Select(pair => new TranscriptCategory(pair.Key,
            pair.Value.Label, pair.Value.AllyValue, pair.Value.TypeColor.ToArgb())).ToArray(),
        CombatantData.IncomingDamageTypeDataObjects.Select(pair => new TranscriptCategory(pair.Key,
            pair.Value.Label, pair.Value.AllyValue, pair.Value.TypeColor.ToArgb())).ToArray(),
        CombatantData.SwingTypeToDamageTypeDataLinksOutgoing.Select(pair => new TranscriptLink(pair.Key, pair.Value.ToArray())).ToArray(),
        CombatantData.SwingTypeToDamageTypeDataLinksIncoming.Select(pair => new TranscriptLink(pair.Key, pair.Value.ToArray())).ToArray(),
        new[] { CombatantData.DamageTypeDataNonSkillDamage, CombatantData.DamageTypeDataOutgoingDamage,
            CombatantData.DamageTypeDataOutgoingHealing, CombatantData.DamageTypeDataIncomingDamage,
            CombatantData.DamageTypeDataIncomingHealing, CombatantData.DamageTypeDataOutgoingPowerReplenish,
            CombatantData.DamageTypeDataOutgoingPowerDamage, CombatantData.DamageTypeDataOutgoingCures },
        CombatantData.DamageSwingTypes.ToArray(), CombatantData.HealingSwingTypes.ToArray(),
        ActGlobals.restrictToAll, ActGlobals.longDuration, ActGlobals.blockIsHit);

    private static void Install(TranscriptConfiguration config)
    {
        CultureInfo.CurrentCulture = CultureInfo.GetCultureInfo(config.Culture);
        CultureInfo.CurrentUICulture = CultureInfo.GetCultureInfo(config.UICulture);
        // Dictionary insertion order selects ACT's final All category; never sort or deduplicate these source arrays.
        CombatantData.OutgoingDamageTypeDataObjects = Definitions(config.Outgoing);
        CombatantData.IncomingDamageTypeDataObjects = Definitions(config.Incoming);
        CombatantData.SwingTypeToDamageTypeDataLinksOutgoing = Links(config.OutgoingLinks);
        CombatantData.SwingTypeToDamageTypeDataLinksIncoming = Links(config.IncomingLinks);
        var names = config.CategoryNames;
        CombatantData.DamageTypeDataNonSkillDamage = names[0];
        CombatantData.DamageTypeDataOutgoingDamage = names[1];
        CombatantData.DamageTypeDataOutgoingHealing = names[2];
        CombatantData.DamageTypeDataIncomingDamage = names[3];
        CombatantData.DamageTypeDataIncomingHealing = names[4];
        CombatantData.DamageTypeDataOutgoingPowerReplenish = names[5];
        CombatantData.DamageTypeDataOutgoingPowerDamage = names[6];
        CombatantData.DamageTypeDataOutgoingCures = names[7];
        CombatantData.DamageSwingTypes = new(config.DamageSwingTypes);
        CombatantData.HealingSwingTypes = new(config.HealingSwingTypes);
        ActGlobals.restrictToAll = config.RestrictToAll;
        ActGlobals.longDuration = config.LongDuration;
        ActGlobals.blockIsHit = config.BlockIsHit;
    }

    private static Dictionary<string, CombatantData.DamageTypeDef> Definitions(TranscriptCategory[] values)
    {
        var result = new Dictionary<string, CombatantData.DamageTypeDef>();
        foreach (var value in values) result.Add(value.Key, new(value.Label, value.AllyValue, Color.FromArgb(value.Argb)));
        return result;
    }

    private static SortedDictionary<int, List<string>> Links(TranscriptLink[] values)
    {
        var result = new SortedDictionary<int, List<string>>();
        foreach (var value in values) result.Add(value.SwingType, new(value.Categories));
        return result;
    }

    private static void Check([System.Diagnostics.CodeAnalysis.DoesNotReturnIf(false)] bool condition, string detail)
    {
        if (!condition) throw new InvalidDataException("Genuine recorded-action replay mismatch: " + detail);
    }
}
