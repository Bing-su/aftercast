namespace Aftercast.ActCompat
{
    public enum CombatSnapshotStatus { Ended = 0, Importing = 1, PartialImport = 2 }

    // Query-owned values retain ACT's signed totals, nonfinite rates and original DateTime kinds.
    public sealed record CombatantSnapshotData(string Name, bool IsAlly, long Damage, long Healed,
        long DamageTaken, long HealsTaken, long DurationTicks, double DPS, double EncDPS, double EncHPS,
        float ToHit, int Swings, int Hits, int CritHits, int Heals, int Misses, int Blocked, int Kills, int Deaths);

    public sealed record CombatSnapshotData(ulong Revision, bool IsImport, CombatSnapshotStatus Status,
        string Zone, string Title, string Character, DateTime Start, DateTime End, long DurationTicks,
        long Damage, long Healed, CombatantSnapshotData[] Combatants);
}

namespace Advanced_Combat_Tracker
{
    public partial class FormActMain
    {
        private Aftercast.ActCompat.CombatSnapshotData? latestCombatSnapshot;
        private System.Runtime.ExceptionServices.ExceptionDispatchInfo? combatSnapshotFailure;
        private ulong combatSnapshotRevision;
        private bool combatSnapshotImport;
        private Aftercast.ActCompat.CombatSnapshotStatus combatSnapshotStatus;

        private void PublishCombatSnapshotState(bool import, EncounterData encounter)
        {
            lock (CombatGate)
            {
                var next = checked(combatSnapshotRevision + 1);
                combatSnapshotImport = import;
                combatSnapshotStatus = import ? Aftercast.ActCompat.CombatSnapshotStatus.Importing
                    : Aftercast.ActCompat.CombatSnapshotStatus.Ended;
                combatSnapshotRevision = next;
                latestCombatSnapshot = null;
                combatSnapshotFailure = null;
                try { latestCombatSnapshot = CaptureCombatSnapshot(encounter); }
                catch (Exception exception)
                {
                    // Snapshot extraction is optional: preserve its failure for query without failing combat finalization.
                    combatSnapshotFailure = System.Runtime.ExceptionServices.ExceptionDispatchInfo.Capture(exception);
                }
            }
        }

        private ulong GetCombatSnapshotRevision()
        {
            lock (CombatGate) return combatSnapshotRevision;
        }

        private void CompleteImportSnapshotState(ulong before, bool success)
        {
            lock (CombatGate)
            {
                // A later live completion, or an import with no new encounter, must keep its existing result.
                if (combatSnapshotRevision <= before || !combatSnapshotImport ||
                    combatSnapshotStatus != Aftercast.ActCompat.CombatSnapshotStatus.Importing) return;
                var next = checked(combatSnapshotRevision + 1);
                combatSnapshotStatus = success ? Aftercast.ActCompat.CombatSnapshotStatus.Ended
                    : Aftercast.ActCompat.CombatSnapshotStatus.PartialImport;
                combatSnapshotRevision = next;
                // Import completion changes status only; public ACT mutations cannot rewrite an existing version.
                if (latestCombatSnapshot is { } snapshot)
                    latestCombatSnapshot = snapshot with { Revision = next, Status = combatSnapshotStatus };
            }
        }

        internal Aftercast.ActCompat.CombatSnapshotData? ReadCombatSnapshot(ulong afterRevision)
        {
            lock (CombatGate)
            {
                if (afterRevision > combatSnapshotRevision)
                    throw new ArgumentOutOfRangeException(nameof(afterRevision), "The requested revision is newer than the host snapshot.");
                if (combatSnapshotRevision == 0 || afterRevision == combatSnapshotRevision) return null;
                combatSnapshotFailure?.Throw();
                var snapshot = latestCombatSnapshot!;
                // Each caller owns its array; replacing an entry must not corrupt future reads of the same revision.
                return snapshot with { Combatants = (Aftercast.ActCompat.CombatantSnapshotData[])snapshot.Combatants.Clone() };
            }
        }

        private Aftercast.ActCompat.CombatantSnapshotData[] CaptureCombatants(EncounterData encounter)
        {
            if (encounter.Items.Count > 65536)
                throw new InvalidOperationException("Combat snapshots support at most 65536 actors.");
            var allies = encounter.GetAllies();
            var actors = new Aftercast.ActCompat.CombatantSnapshotData[encounter.Items.Count];
            for (var index = 0; index < actors.Length; index++)
            {
                var actor = encounter.Items.Values[index];
                actors[index] = new(actor.Name, allies.Contains(actor), actor.Damage, actor.Healed,
                    actor.DamageTaken, actor.HealsTaken, actor.Duration.Ticks, actor.DPS, actor.EncDPS,
                    actor.EncHPS, actor.Items[CombatantData.DamageTypeDataOutgoingDamage].ToHit,
                    actor.Swings, actor.Hits, actor.CritHits, actor.Heals, actor.Misses, actor.Blocked,
                    actor.Kills, actor.Deaths);
            }
            return actors;
        }

        private Aftercast.ActCompat.CombatSnapshotData CaptureCombatSnapshot(EncounterData encounter)
        {
            var actors = CaptureCombatants(encounter);
            // Freeze actual getters once after end notifications, before public ACT state can later change.
            return new(combatSnapshotRevision, combatSnapshotImport, combatSnapshotStatus,
                encounter.ZoneName, encounter.Title, encounter.CharName, encounter.StartTime, encounter.EndTime,
                encounter.Duration.Ticks, encounter.Damage, encounter.Healed, actors);
        }
    }
}
