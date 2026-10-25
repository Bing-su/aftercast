using System.Reflection;
using System.Runtime.CompilerServices;
using Advanced_Combat_Tracker;

namespace Aftercast.LegacyReference;

public static class HistoryRecordProbe
{
    public static string Run()
    {
        var root = (FormActMain)RuntimeHelpers.GetUninitializedObject(typeof(FormActMain));
        GC.SuppressFinalize(root);
        root.ZoneDatabase = new(); root.EncDatabase = new();
        var translations = typeof(FormActMain).GetField("Trans", BindingFlags.Instance | BindingFlags.NonPublic);
        if (translations is not null) translations.SetValue(root, Activator.CreateInstance(translations.FieldType, nonPublic: true));
        var epoch = DateTime.UnixEpoch;
        root.EncDatabaseAdd(new(1, epoch, DateTime.MinValue, "First", "Source", "logs"));
        if (root.EncDatabase[epoch].EndTime != epoch) throw new Exception("Unset encounter end differs");
        root.EncDatabaseAdd(new(1, epoch, epoch.AddSeconds(2), "Second", "Source", "logs"));
        if (root.EncDatabase.Count != 1 || root.EncDatabase[epoch].Label != "Second") throw new Exception("Duplicate encounter replacement differs");
        root.ZoneDatabaseAdd(new(0, epoch, DateTime.MaxValue, "Fixture", "Source", "logs"));
        if (root.ZoneDatabase[epoch].Duration != TimeSpan.FromDays(1)) throw new Exception("Unclosed zone duration differs");
        root.ZoneDatabaseAdd(new(0, epoch, epoch.AddSeconds(10), "Bounded", "Source", "logs"));
        root.ZoneDatabaseAdd(new(0, epoch, epoch.AddSeconds(5), "Shorter", "Source", "logs"));
        root.ZoneDatabaseAdd(new(0, epoch.AddDays(1), epoch, "Invalid", "Source", "logs"));
        root.ZoneDatabaseAdd(new(0, epoch.AddDays(2), epoch.AddDays(3), "Import Zone", "Source", "logs"));
        if (root.ZoneDatabase.Count != 1 || root.ZoneDatabase[epoch].Label != "Bounded") throw new Exception("Zone replacement/filter policy differs: " + string.Join(",", root.ZoneDatabase.Values.Select(record => record.Label)));
        return "History record policies";
    }
}
