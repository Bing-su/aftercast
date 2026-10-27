namespace Advanced_Combat_Tracker;

public partial class FormActMain
{
    // Public ACT collections can bypass Root mutation guards; bind the original selected topology too.
    private sealed class TextReportScope
    {
        internal readonly EncounterData Encounter;
        private readonly FormActMain root;
        private readonly List<ZoneData> zones;
        private readonly int zoneIndex,encounterIndex,zoneCount,encounterCount;
        private readonly ZoneData zone;
        private readonly bool populateAll;
        private readonly List<EncounterData> encounters;
        private readonly SortedList<string,CombatantData> actors;
        private readonly KeyValuePair<string,CombatantData>[] members;
        private readonly string character,title,zoneName;
        private readonly bool selective,ignoreEnemies;
        private CombatantData[]? allies;
        private List<CombatantData>? boundAllies;
        private bool allActors;
        internal TextReportScope(FormActMain root,int zoneIndex,int encounterIndex,EncounterData encounter)
        {
            this.root=root;this.zoneIndex=zoneIndex;this.encounterIndex=encounterIndex;Encounter=encounter;
            zones=root.combatZones;zoneCount=zones.Count;zone=zones[zoneIndex];populateAll=zone.PopulateAll;
            encounters=zone.Items;encounterCount=encounters.Count;actors=encounter.Items;
            character=encounter.CharName;title=encounter.Title;zoneName=encounter.ZoneName;
            selective=encounter.GetIsSelective();ignoreEnemies=encounter.GetIgnoreEnemies();
            members=actors.Take(16385).ToArray();
            if(members.Length>16384||members.Any(pair=>pair.Value is null))throw new InvalidDataException("Invalid or oversized report combatant scope");
        }
        internal void BindAllies(List<CombatantData> selected,bool allActors)
        {this.allActors=allActors;boundAllies=selected;allies=selected.ToArray();}
        internal void Check()
        {
            if(!ReferenceEquals(root.combatZones,zones)||zones.Count!=zoneCount||!ReferenceEquals(zones[zoneIndex],zone)||zone.PopulateAll!=populateAll||
                !ReferenceEquals(zone.Items,encounters)||encounters.Count!=encounterCount||!ReferenceEquals(encounters[encounterIndex],Encounter)||
                Encounter.CharName!=character||Encounter.Title!=title||Encounter.ZoneName!=zoneName||Encounter.GetIsSelective()!=selective||Encounter.GetIgnoreEnemies()!=ignoreEnemies||
                allies is not null&&(!boundAllies!.SequenceEqual(allies,ReferenceEqualityComparer.Instance)||!(allActors?Encounter.Items.Values:Encounter.GetAllies()).SequenceEqual(allies,ReferenceEqualityComparer.Instance))||
                !ReferenceEquals(Encounter.Items,actors)||actors.Count!=members.Length||
                members.Any(pair=>!actors.TryGetValue(pair.Key,out var actor)||!ReferenceEquals(pair.Value,actor)))
                throw new InvalidOperationException("Selected report scope changed during rendering");
        }
    }
    private TextReportScope? FindTextReportScope(ulong id)
    {
        var scanned=0;
        if(combatZones.Count>16384)throw new InvalidDataException("Report zone scan exceeds 16384 slots");
        for(var zoneIndex=0;zoneIndex<combatZones.Count;zoneIndex++)
        {
            var zone=combatZones[zoneIndex];
            for(var index=zone.PopulateAll?1:0;index<zone.Items.Count;index++)
            {
                if(++scanned>16384)throw new InvalidDataException("Report encounter scan exceeds 16384 slots");
                var encounter=zone.Items[index];
                if(Identity(encounter).Id==id)return new(this,zoneIndex,index,encounter);
            }
        }
        return null;
    }
}
