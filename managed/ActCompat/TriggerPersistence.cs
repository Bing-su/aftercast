using System.Text;
using System.Xml;

namespace Advanced_Combat_Tracker;

public partial class FormActMain
{
    internal void LoadCustomTriggers(string path)
    {
        if (!File.Exists(path)) return;
        var loaded = ReadCustomTriggers(path);
        // Validate the whole file before publishing any definitions, e.g. an invalid last regex preserves the old registry.
        lock (triggerGate)
        {
            CustomTriggers.Clear();
            foreach (var (key, trigger) in loaded) CustomTriggers.Add(key, trigger);
        }
        RebuildActiveCustomTriggers();
    }

    private static SortedList<string, CustomTrigger> ReadCustomTriggers(string path)
    {
        // Read file streams so canonical Windows paths also work after a host restart.
        using var stream = File.OpenRead(path);
        using var reader = XmlReader.Create(stream, new XmlReaderSettings
        { DtdProcessing = DtdProcessing.Prohibit, XmlResolver = null, MaxCharactersInDocument = 4 * 1024 * 1024 });
        var document = new XmlDocument { XmlResolver = null };
        document.Load(reader);
        if (document.DocumentElement?.Name != "CustomTriggers") throw new XmlException("Expected CustomTriggers root");
        var loaded = new SortedList<string, CustomTrigger>();
        foreach (XmlNode node in document.DocumentElement.ChildNodes)
        {
            if (node is not XmlElement element || element.Name != "Trigger") continue;
            string Value(string name, string fallback = "") => element.HasAttribute(name) ? element.GetAttribute(name) : fallback;
            var trigger = new CustomTrigger(Value("Regex"), Value("Category"))
            {
                Active = bool.Parse(Value("Active", "true")), SoundType = int.Parse(Value("SoundType", "0")),
                SoundData = Value("SoundData"), RestrictToCategoryZone = bool.Parse(Value("CategoryRestrict", "false")),
                Timer = bool.Parse(Value("Timer", "false")), TimerName = Value("TimerName"), Tabbed = bool.Parse(Value("Tabbed", "false"))
            };
            loaded[trigger.Key] = trigger;
        }
        return loaded;
    }

    internal void SaveCustomTriggers(string path)
    {
        lock (triggerGate) if (CustomTriggers.Count == 0 && !File.Exists(path)) return;
        var temporary = path + "." + Guid.NewGuid().ToString("N") + ".tmp";
        try
        {
            using (var stream = new FileStream(temporary, FileMode.CreateNew, FileAccess.Write, FileShare.None))
            {
                using (var writer = new XmlTextWriter(stream, new UTF8Encoding(false)))
                {
                    SaveXmlCustomTriggers(writer);
                    writer.Flush();
                    if (stream.Length > 4 * 1024 * 1024) throw new InvalidDataException("Trigger settings exceed the 4 MiB load limit");
                    stream.Flush(true);
                }
            }
            // Reject caller-edited invalid patterns/XML before replacing a previously readable settings file.
            _ = ReadCustomTriggers(temporary);
            File.Move(temporary, path, overwrite: true);
        }
        finally { if (File.Exists(temporary)) File.Delete(temporary); }
    }
}
