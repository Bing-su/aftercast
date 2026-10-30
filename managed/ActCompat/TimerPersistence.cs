using System.Text;
using System.Xml;
using System.Xml.Serialization;
using Aftercast.ActCompat;

namespace Advanced_Combat_Tracker;

public partial class FormSpellTimers
{
    private static readonly XmlSerializer definitionsSerializer = new(typeof(TimerDefinitionsFile));
    private TimerDefinitionsFile ReadTimerFile(string path)
    {
        using var stream = File.OpenRead(path);
        using var reader = XmlReader.Create(stream,new XmlReaderSettings
        { DtdProcessing = DtdProcessing.Prohibit, XmlResolver = null, MaxCharactersInDocument = 4 * 1024 * 1024 });
        var document = new XmlDocument { XmlResolver = null };
        document.Load(reader);
        // ACT uses Boolean.ToString/Parse, including True/False; XmlSerializer alone accepts a different lexical set.
        var booleanNames = new HashSet<string> { "OnlyMasterTicks", "RestrictCategory", "Panel1", "Panel2", "Modable", "Checked", "RadialDisplay", "Absolute", "Restrict" };
        if (document.DocumentElement is { } root)
            foreach (XmlNode node in root.ChildNodes)
                if (node is XmlElement { Name: "Spell" } spell)
                    foreach (XmlAttribute attribute in spell.Attributes)
                    {
                        if (!booleanNames.Contains(attribute.Name)) continue;
                        if (!bool.TryParse(attribute.Value,out var value)) throw new XmlException("Invalid timer boolean: " + attribute.Name);
                        attribute.Value = value ? "true" : "false";
                    }
        using var normalized = new XmlNodeReader(document);
        var file = (TimerDefinitionsFile?)definitionsSerializer.Deserialize(normalized)
            ?? throw new XmlException("Missing SpellTimers root");
        // XmlSerializer returns null for an absent array, including a valid empty SpellTimers file.
        file.Items ??= [];
        var keys = new SortedSet<string>(TimerDefs.Comparer);
        foreach (var definition in file.Items)
        {
            ValidateTimerText(definition);
            if (!keys.Add(definition.ToData().Key)) throw new InvalidDataException("Duplicate timer key in settings");
        }
        return file;
    }
    internal void LoadSpellTimers(string path)
    {
        if (!File.Exists(path)) return;
        var file = ReadTimerFile(path);
        lock (gate) PublishTimerFile(file);
    }
    private void PublishTimerFile(TimerDefinitionsFile file)
    {
        var replacement = new SortedDictionary<string,TimerData>(TimerDefs.Comparer);
        var updates = new List<(TimerDefinition Definition,TimerData Existing)>();
        foreach (var definition in file.Items)
        {
            var data = definition.ToData();
            var key = data.Key;
            // Prepare every culture-compared key before mutating any existing frame's metadata.
            if (TimerDefs.TryGetValue(key,out var existing)) { updates.Add((definition,existing)); data = existing; }
            replacement.Add(key,data);
        }
        // ACT XML load reuses registry objects; captured countdown duration stays unchanged.
        foreach (var (definition,existing) in updates) definition.Apply(existing);
        TimerDefs.Clear();
        foreach (var pair in replacement) TimerDefs.Add(pair.Key,pair.Value);
        RebuildSpellTreeView();
    }
    internal void SaveSpellTimers(string path)
    {
        lock (gate)
        {
            if (TimerDefs.Count == 0 && !File.Exists(path)) return;
            WriteTimerFile(path,TimerDefs.Values);
        }
    }
    private void WriteTimerFile(string path,IEnumerable<TimerData> definitions)
    {
        using var lease = MixedConfigFiles.AcquireSingle(path);
        var file = new TimerDefinitionsFile { Items = definitions.Select(TimerDefinition.FromData).ToArray() };
        foreach (var definition in file.Items) ValidateTimerText(definition);
        var temporary = path + "." + Guid.NewGuid().ToString("N") + ".tmp";
        try
        {
            using (var stream = new FileStream(temporary,FileMode.CreateNew,FileAccess.Write,FileShare.None))
            {
                using var writer = XmlWriter.Create(stream,new XmlWriterSettings { Encoding = new UTF8Encoding(false), CloseOutput = false });
                var namespaces = new XmlSerializerNamespaces(); namespaces.Add("","");
                definitionsSerializer.Serialize(writer,file,namespaces);
                writer.Flush();
                if (stream.Length > 4 * 1024 * 1024) throw new InvalidDataException("Timer settings exceed the 4 MiB load limit");
                stream.Flush(true);
            }
            _ = ReadTimerFile(temporary);
            File.Move(temporary,path,overwrite:true);
        }
        finally { if (File.Exists(temporary)) File.Delete(temporary); }
    }
}
