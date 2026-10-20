using Advanced_Combat_Tracker;

internal static class CombatPolicyChecks
{
    private static void Check(bool condition)
    {
        if (!condition) throw new Exception("Combat policy boundary check failed");
    }

    private static void Unsupported(Action action)
    {
        try { action(); }
        catch (NotSupportedException) { return; }
        throw new Exception("Unsupported combat policy appeared to succeed");
    }

    internal static void Run()
    {
        // Fail before mutation so callers can correct a policy without retaining half-applied actions.
        var zone = new ZoneData(DateTime.UnixEpoch, "Fixture", true, true, false);
        var encounter = zone.Items[0];
        var action = new MasterSwing(1, false, "", new Dnum(100), DateTime.UnixEpoch, 1,
            "Strike", "Source", "Physical", "Target");
        Unsupported(() => zone.AddCombatAction(action));
        Unsupported(() => encounter.AddCombatAction(action));
        Unsupported(() => encounter.AddReverseCombatAction(action));
        Check(!encounter.Active && encounter.StartTimes.Count == 0 && encounter.Items.Count == 0);
        Check(action.ParentEncounter is null);

        var plain = new EncounterData("Source", "Fixture", zone);
        Check(plain.GetAllies().Count == 0);
        Check(!plain.Active && plain.EndTimes.Count == 0);
        var actor = new CombatantData("Source", plain);
        var allies = new List<CombatantData> { actor };
        plain.SetAllies(allies);
        Check(ReferenceEquals(plain.GetAllies(), allies));
        plain.SetAllies([]);
        Check(plain.GetAllies().Count == 0);
        plain.SetAllies(allies);
        plain.SetAllies(null);
        Check(plain.GetAllies().Count == 0);
        Console.WriteLine("Unsupported selection rejects before graph mutation; automatic and explicit allies preserve ownership.");
    }
}
