using System.Collections;
using System.Reflection;
using System.Runtime.CompilerServices;
using Advanced_Combat_Tracker;

namespace Aftercast.LegacyReference;

public static class DataCorrectionProbe
{
    // Compare private headless rules because ACT exposes their editor rather than a public configuration API.
    public static string Run()
    {
        var root = (FormActMain)RuntimeHelpers.GetUninitializedObject(typeof(FormActMain));
        GC.SuppressFinalize(root);
        var rootType = typeof(FormActMain);
        object Rule(string name, params object[] args) => Activator.CreateInstance(rootType.GetNestedType(name, BindingFlags.NonPublic)!, args)!;
        IDictionary Install(string name, string type)
        {
            var list = (IDictionary)Activator.CreateInstance(typeof(SortedList<,>).MakeGenericType(typeof(string), rootType.GetNestedType(type, BindingFlags.NonPublic)!))!;
            rootType.GetField(name, BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(root, list);
            return list;
        }
        void Call(string method, CombatActionEventArgs info) => rootType.GetMethod(method, BindingFlags.Instance | BindingFlags.NonPublic)!.Invoke(root, [info]);
        var renames = Install("renameList", "RenameFix");
        var redirects = Install("redirectList", "RedirectFix");
        var first = Rule("RenameFix", "Pet", "Owner");
        var second = Rule("RenameFix", "Owner", "Player");
        renames.Add(first.ToString()!, first);
        renames.Add(second.ToString()!, second);
        var damage = CombatantData.DamageSwingTypes;
        var healing = CombatantData.HealingSwingTypes;
        try
        {
            CombatantData.DamageSwingTypes = [1]; CombatantData.HealingSwingTypes = [3];
            foreach (var type in new[] { 0, 1, 2, 3, 9 })
                foreach (var swing in new[] { 1, 3, 20 })
                {
                    var rule = Rule("RedirectFix", "Strike", "Redirected", type);
                    redirects.Clear(); redirects.Add(rule.ToString()!, rule);
                    var action = new MasterSwing(swing, false, "", new Dnum(10), DateTime.UnixEpoch, 1, "Strike", "Pet", "Physical", "Pet");
                    var info = new CombatActionEventArgs(action);
                    Call("RenameCombatant", info);
                    // Owner sorts before Pet: it was already visited when Pet became Owner, so the chain stops here.
                    if (info.attacker != "Owner" || info.victim != "Owner") throw new Exception("Rename ordering differs");
                    Call("RedirectAbility", info);
                    var matches = type switch { 1 => swing == 1, 2 => swing == 3, 3 => swing == 20, _ => true };
                    if (info.attacker != (matches ? "Redirected" : "Owner") || info.victim != "Owner" || action.Attacker != "Pet")
                        throw new Exception("Redirect or snapshot behavior differs");
                    rule.GetType().GetProperty("Active")!.SetValue(rule, false);
                    info.attacker = "Owner";
                    Call("RedirectAbility", info);
                    if (info.attacker != "Owner") throw new Exception("Disabled redirect applied");
                }
            first.GetType().GetProperty("Active")!.SetValue(first, false);
            var disabled = new CombatActionEventArgs(new MasterSwing(1, false, "", new Dnum(1), DateTime.UnixEpoch, 1, "Other", "Pet", "Physical", "pet"));
            Call("RenameCombatant", disabled);
            if (disabled.attacker != "Pet" || disabled.victim != "pet") throw new Exception("Disabled/case-sensitive rename differs");
            return "Data corrections";
        }
        finally { CombatantData.DamageSwingTypes = damage; CombatantData.HealingSwingTypes = healing; }
    }
}
