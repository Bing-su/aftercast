using System.Windows.Forms;
using System.Xml;
using System.Reflection;

namespace Advanced_Combat_Tracker;

public class SettingsSerializer(object parent) : IDisposable
{
    private readonly Dictionary<string, Control> controls = new(StringComparer.Ordinal);
    private readonly Dictionary<ComboBox, ComboBoxStyle> comboStyles = new(ReferenceEqualityComparer.Instance);

    private readonly Dictionary<string, List<string>> settings = new()
    {
        ["String"] = new(), ["Int32"] = new(), ["Int64"] = new(), ["Boolean"] = new()
    };
    private const BindingFlags ParentMembers = BindingFlags.Instance | BindingFlags.Public | BindingFlags.NonPublic;

    public void AddStringSetting(string name) => Register("String", name);
    public void AddIntSetting(string name) => Register("Int32", name);
    public void AddLongSetting(string name) => Register("Int64", name);
    public void AddBooleanSetting(string name) => Register("Boolean", name);
    private void Register(string type, string name)
    {
        ArgumentNullException.ThrowIfNull(name);
        if (!settings[type].Contains(name)) settings[type].Add(name);
    }

    public bool RemoveControlSetting(string name) => controls.Remove(name);

    private object? GetParentValue(string name) => parent.GetType().GetField(name, ParentMembers) is { } field
        ? field.GetValue(parent) : parent.GetType().GetProperty(name, ParentMembers)!.GetValue(parent);

    private void SetParentValue(string name, object value)
    {
        if (parent.GetType().GetField(name, ParentMembers) is { } field) field.SetValue(parent, value);
        else parent.GetType().GetProperty(name, ParentMembers)!.SetValue(parent, value);
    }

    public void AddControlSetting(string name, Control control)
    {
        ArgumentNullException.ThrowIfNull(name);
        if (controls.ContainsKey(name)) return;
        ArgumentNullException.ThrowIfNull(control);
        var type = control.GetType();
        if (type != typeof(CheckBox) && type != typeof(ComboBox) && type != typeof(TextBox))
            throw new ArgumentException($"Cannot serialize control of type {type.FullName}", nameof(control));
        controls.TryAdd(name, control);
    }

    public int ImportFromXml(XmlTextReader reader)
    {
        ArgumentNullException.ThrowIfNull(reader);
        comboStyles.Clear();
        // Reject DTDs before consuming the wrapper; settings must never fetch external resources.
        reader.XmlResolver = null;
        reader.DtdProcessing = DtdProcessing.Prohibit;
        var document = new XmlDocument { XmlResolver = null };
        document.LoadXml(reader.ReadOuterXml());
        var errors = 0;
        foreach (XmlNode node in document.DocumentElement!.ChildNodes)
        {
            if (node.Name is not ("CheckBox" or "ComboBox" or "TextBox") && !settings.ContainsKey(node.Name)) continue;
            try
            {
                var name = Attribute(node, "Name");
                var value = Attribute(node, "Value");
                if (settings.TryGetValue(node.Name, out var names))
                {
                    // Only registered settings may change private parent state, e.g. an XML Name='password' cannot opt itself in.
                    if (!names.Contains(name)) throw new ArgumentException("Parent setting is not registered: " + name);
                    SetParentValue(name, node.Name switch
                    {
                        "Int32" => int.Parse(value), "Int64" => long.Parse(value), "Boolean" => bool.Parse(value), _ => value
                    });
                    continue;
                }
                var control = controls[name];
                switch (node.Name)
                {
                    case "CheckBox":
                        ((CheckBox)control).Checked = bool.Parse(value);
                        break;
                    case "TextBox":
                        ((TextBox)control).Text = value;
                        break;
                    case "ComboBox":
                        var combo = (ComboBox)control;
                        // Registration keys and Control.Name can differ; retain the actual control for restoration.
                        comboStyles.Add(combo, combo.DropDownStyle);
                        combo.DropDownStyle = ComboBoxStyle.DropDown;
                        combo.Text = value;
                        break;
                }
            }
            catch (Exception error)
            {
                errors++;
                ActGlobals.oFormActMain?.WriteExceptionLog(error, node.OuterXml);
            }
        }
        return errors;
    }

    public void ExportToXml(XmlTextWriter writer)
    {
        ArgumentNullException.ThrowIfNull(writer);
        foreach (var (name, control) in controls)
        {
            var value = control is CheckBox check ? check.Checked.ToString() : control.Text;
            writer.WriteStartElement(control.GetType().Name);
            writer.WriteAttributeString("Name", name);
            writer.WriteAttributeString("Value", value);
            writer.WriteEndElement();
        }
        foreach (var (type, names) in settings)
            foreach (var name in names)
            {
                // Resolve before writing an element so a missing member cannot corrupt following settings.
                string? value;
                try { value = GetParentValue(name)?.ToString(); }
                catch (Exception error) { ActGlobals.oFormActMain?.WriteExceptionLog(error, "SettingsSerializer.ExportToXml: " + name); continue; }
                writer.WriteStartElement(type);
                writer.WriteAttributeString("Name", name);
                writer.WriteAttributeString("Value", value);
                writer.WriteEndElement();
            }
    }

    public void FinializeComboBoxes()
    {
        foreach (var (combo, style) in comboStyles)
        {
            try
            {
                var text = combo.Text;
                combo.DropDownStyle = style;
                for (var index = 0; index < combo.Items.Count; index++)
                    if (combo.Items[index].ToString() == text) combo.SelectedIndex = index;
            }
            catch (Exception error)
            {
                // Preserve best-effort restoration while making adapter failures observable.
                ActGlobals.oFormActMain?.WriteExceptionLog(error, "SettingsSerializer.FinializeComboBoxes");
            }
        }
    }

    public void Dispose() => FinializeComboBoxes();

    private static string Attribute(XmlNode node, string name) =>
        node.Attributes?[name]?.Value ?? throw new NullReferenceException($"The attribute, {name} does not exist.");
}
