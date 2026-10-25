using System.Reflection;
using System.Runtime.CompilerServices;
using System.Text;
using System.Xml;
using Advanced_Combat_Tracker;
using Aftercast.Testing;

namespace Aftercast.LegacyReference;

public static class EncounterXmlProbe
{
    public static string RunImport()
    {
        CombatFixture.ConfigureCategories();
        var root = (FormActMain)RuntimeHelpers.GetUninitializedObject(typeof(FormActMain));
        GC.SuppressFinalize(root);
        var bytes = Encoding.UTF8.GetBytes("<Encounters><Encounter Name='Fixture' CharName='Source'><StartTime>2026-10-25T12:00:00</StartTime><EndTime>2026-10-25T12:00:02</EndTime><Swing SwingType='1' Critical='True' Time='2026-10-25T12:00:02' TimeSorter='19' Combatant='Source' Victim='Target' AttackType='Strike' DamageType='Physical' Damage='1,234' Special='Special'/><Ally>Source</Ally></Encounter></Encounters>");
        var stream = new MemoryStream(bytes);
        var encounters = root.XmlToEncounters(stream);
        if (encounters.Count != 1 || encounters[0].Damage != 1234 || encounters[0].Title != "Fixture" || encounters[0].ZoneName != "Import" || stream.CanRead)
            throw new Exception("ACT XML import or input ownership differs");
        return "Encounter XML import";
    }
    // Execute the original exporter without opening ACT's main window; output is independently checked as XML.
    public static string Run()
    {
        CombatFixture.ConfigureCategories();
        var root = (FormActMain)RuntimeHelpers.GetUninitializedObject(typeof(FormActMain));
        GC.SuppressFinalize(root);
        var translation = typeof(FormActMain).GetField("Trans", BindingFlags.Instance | BindingFlags.NonPublic);
        if (translation is not null) translation.SetValue(root, Activator.CreateInstance(translation.FieldType, nonPublic: true));
        var epoch = new DateTime(2026, 10, 25, 12, 0, 0, DateTimeKind.Unspecified);
        var zone = new ZoneData(epoch, "Fixture & Zone", false, false, false);
        var encounter = new EncounterData("Source", zone.ZoneName, zone) { Title = "Enemy <test>" };
        encounter.StartTimes.Add(epoch); encounter.EndTimes.Add(epoch.AddSeconds(2));
        var action = new MasterSwing(1, true, "Special", new Dnum(1234), epoch.AddSeconds(2), 19, "Strike", "Source", "Physical", "Target");
        action.Tags["fixture"] = "A&B";
        encounter.AddCombatAction(action);
        encounter.SetAllies([encounter.GetCombatant("Source")]);
        encounter.LogLines.Add(new(epoch, "original <line>", 12, 20));
        using var stream = new MemoryStream();
        root.EncountersToXml([encounter], stream, true);
        if (!stream.CanWrite) throw new Exception("Exporter closed caller stream");
        var bytes = stream.ToArray();
        var document = new XmlDocument();
        document.LoadXml(Encoding.UTF8.GetString(bytes).TrimStart('\ufeff'));
        var item = (XmlElement)document.SelectSingleNode("/Encounters/Encounter")!;
        if (item.GetAttribute("Name") != "Enemy <test>" || item.GetAttribute("ZoneName") != "Fixture & Zone" ||
            item.SelectNodes("Swing")!.Count != 1 || item.SelectSingleNode("Ally")!.InnerText != "Source" ||
            ((XmlElement)item.SelectSingleNode("Swing")!).GetAttribute("Damage") != "1,234" || item.SelectNodes("LogLine")!.Count != 1)
            throw new Exception("ACT encounter XML shape differs");
        var gzip = root.MemCompressGZip(bytes);
        if (!root.MemDecompressGzip(gzip).SequenceEqual(bytes)) throw new Exception("ACT gzip round-trip failed");
        return "Encounter XML export";
    }
}
