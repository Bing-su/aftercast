using System.Xml;
using Advanced_Combat_Tracker;

namespace Aftercast.LegacyReference;

// Compile parent-member references against ACT so plugins can round-trip settings without controls.
public static class ParentSettingsProbe
{
    private sealed class Settings
    {
        private string text = "A&B <한글>";
        public int Count = -123;
        public long Total { get; private set; } = long.MaxValue;
        private bool Enabled { get; set; } = true;
        public bool Matches => text == "A&B <한글>" && Count == -123 && Total == long.MaxValue && Enabled;
        public void Clear() { text = ""; Count = 0; Total = 0; Enabled = false; }
    }

    public static string Run()
    {
        var parent = new Settings();
        using var serializer = new SettingsSerializer(parent);
        serializer.AddStringSetting("text");
        serializer.AddStringSetting("text");
        serializer.AddIntSetting("Count");
        serializer.AddLongSetting("Total");
        serializer.AddBooleanSetting("Enabled");
        using var output = new StringWriter();
        using (var writer = new XmlTextWriter(output))
        {
            writer.WriteStartElement("Settings");
            serializer.ExportToXml(writer);
            writer.WriteEndElement();
        }
        var document = new XmlDocument();
        document.LoadXml(output.ToString());
        if (document.DocumentElement!.ChildNodes.Count != 4) throw new Exception("Duplicate setting was exported");
        parent.Clear();
        using var reader = new XmlTextReader(new StringReader(output.ToString()));
        reader.MoveToContent();
        if (serializer.ImportFromXml(reader) != 0 || !parent.Matches) throw new Exception("Parent settings round-trip failed");
        return "Parent settings";
    }
}
