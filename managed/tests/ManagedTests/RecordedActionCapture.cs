using System.Globalization;
using System.Text.Json;
using Advanced_Combat_Tracker;
using Aftercast.TestContracts;

// Test-only capture of committed original-plugin submissions. Expected totals must come from the independent action oracle.
internal sealed class RecordedActionCapture : IDisposable
{
    internal sealed record EncounterExpectation(EncounterData Encounter, TranscriptActor[] Actors,
        string[] Allies, long Damage, long Healed);

    private readonly object gate = new();
    private readonly FormActMain root;
    private readonly TranscriptProvenance provenance;
    private readonly TranscriptConfiguration configuration;
    private readonly Dictionary<EncounterData, int> encounters = new(ReferenceEqualityComparer.Instance);
    private readonly List<TranscriptAction> actions = new();
    private bool closed;

    // The caller supplies verified source-artifact digests; this helper does not infer runtime binding from a filename or hash.
    internal static RecordedActionCapture Begin(FormActMain root, TranscriptProvenance provenance) => new(root, provenance);

    private RecordedActionCapture(FormActMain root, TranscriptProvenance provenance)
    {
        ArgumentNullException.ThrowIfNull(root);
        ArgumentNullException.ThrowIfNull(provenance);
        if (root.InCombat) throw new InvalidOperationException("Begin transcript capture before starting the import");
        this.root = root;
        this.provenance = provenance;
        configuration = Configuration();
        root.AfterCombatAction += Observe;
    }

    private void Observe(bool import, CombatActionEventArgs info)
    {
        lock (gate)
        {
            if (closed) return;
            if (!import) throw new InvalidDataException("A live combat action entered an import transcript");
            if (CultureInfo.CurrentCulture.Name != configuration.Culture || CultureInfo.CurrentUICulture.Name != configuration.UICulture)
                throw new InvalidDataException("The action worker culture differs from the captured transcript configuration");
            if (actions.Count >= RecordedActionTranscript.MaximumActions)
                throw new InvalidDataException("Recorded-action capture exceeded the schema action limit");
            // Event-copy fields are mutable; preserve the actual object's constructor scalars after ACT committed Before changes.
            var action = info.combatAction;
            var encounter = action.ParentEncounter ?? throw new InvalidDataException("Committed action has no encounter identity");
            if (!encounters.TryGetValue(encounter, out var id)) encounters.Add(encounter, id = checked(encounters.Count + 1));
            actions.Add(new(id, action.SwingType, action.Critical, action.Special,
                action.Damage.Number, action.Damage.DamageString, action.Time.Ticks, (int)action.Time.Kind,
                action.TimeSorter, action.AttackType, action.Attacker, action.DamageType, action.Victim));
        }
    }

    // Call only after ImportLog returned and drained its worker. For example, Complete(oracleEncounters).Write(privatePath).
    internal RecordedActionTranscript Complete(IReadOnlyList<EncounterExpectation> expected)
    {
        ArgumentNullException.ThrowIfNull(expected);
        lock (gate)
        {
            if (closed) throw new InvalidOperationException("Recorded-action capture is already closed");
            try
            {
                if (root.InCombat) throw new InvalidOperationException("Complete transcript capture only after import completion");
                if (JsonSerializer.Serialize(configuration) != JsonSerializer.Serialize(Configuration()))
                    throw new InvalidDataException("ACT registrations, flags or culture changed during recorded-action capture");
                if (expected.Count != encounters.Count)
                    throw new InvalidDataException("Oracle encounter count differs from the captured action graph");
                var records = new TranscriptEncounter[expected.Count];
                for (var index = 0; index < expected.Count; index++)
                {
                    var value = expected[index];
                    ArgumentNullException.ThrowIfNull(value);
                    var encounter = value.Encounter;
                    if (!encounters.TryGetValue(encounter, out var id) || id != index + 1 || encounter.Active)
                        throw new InvalidDataException("Oracle encounter identity/order differs from the completed action graph");
                    ArgumentNullException.ThrowIfNull(value.Actors);
                    ArgumentNullException.ThrowIfNull(value.Allies);
                    // Copy oracle expectations, never recompute expected numeric totals using the implementation being replayed.
                    records[index] = new(id, encounter.CharName, encounter.ZoneName, encounter.DuplicateDetection,
                        encounter.GetIsSelective(), encounter.GetIgnoreEnemies(), (TranscriptActor[])value.Actors.Clone(),
                        (string[])value.Allies.Clone(), value.Damage, value.Healed);
                }
                var transcript = new RecordedActionTranscript(1, provenance, configuration, records, actions.ToArray());
                transcript.Validate();
                return transcript;
            }
            finally { Close(); }
        }
    }

    private static TranscriptConfiguration Configuration() => new(
        CultureInfo.CurrentCulture.Name, CultureInfo.CurrentUICulture.Name,
        CombatantData.OutgoingDamageTypeDataObjects.Select(pair => new TranscriptCategory(pair.Key,
            pair.Value.Label, pair.Value.AllyValue, pair.Value.TypeColor.ToArgb())).ToArray(),
        CombatantData.IncomingDamageTypeDataObjects.Select(pair => new TranscriptCategory(pair.Key,
            pair.Value.Label, pair.Value.AllyValue, pair.Value.TypeColor.ToArgb())).ToArray(),
        CombatantData.SwingTypeToDamageTypeDataLinksOutgoing.Select(pair => new TranscriptLink(pair.Key, pair.Value.ToArray())).ToArray(),
        CombatantData.SwingTypeToDamageTypeDataLinksIncoming.Select(pair => new TranscriptLink(pair.Key, pair.Value.ToArray())).ToArray(),
        [CombatantData.DamageTypeDataNonSkillDamage, CombatantData.DamageTypeDataOutgoingDamage,
            CombatantData.DamageTypeDataOutgoingHealing, CombatantData.DamageTypeDataIncomingDamage,
            CombatantData.DamageTypeDataIncomingHealing, CombatantData.DamageTypeDataOutgoingPowerReplenish,
            CombatantData.DamageTypeDataOutgoingPowerDamage, CombatantData.DamageTypeDataOutgoingCures],
        CombatantData.DamageSwingTypes.ToArray(), CombatantData.HealingSwingTypes.ToArray(),
        ActGlobals.restrictToAll, ActGlobals.longDuration, ActGlobals.blockIsHit);

    private void Close()
    {
        if (closed) return;
        closed = true;
        root.AfterCombatAction -= Observe;
    }

    public void Dispose()
    {
        lock (gate) Close();
    }
}
