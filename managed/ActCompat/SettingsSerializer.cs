using System.Windows.Forms;
using System.Xml;

namespace Advanced_Combat_Tracker;

public class SettingsSerializer : IDisposable
{
    private readonly Dictionary<string, Control> controls = new(StringComparer.Ordinal);
    private readonly Dictionary<ComboBox, ComboBoxStyle> comboStyles = new(ReferenceEqualityComparer.Instance);

    // Keep the original constructor signature; parent-field settings are outside this control subset.
    public SettingsSerializer(object parent) { }

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
            if (node.Name is not ("CheckBox" or "ComboBox" or "TextBox")) continue;
            try
            {
                var name = Attribute(node, "Name");
                var value = Attribute(node, "Value");
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
