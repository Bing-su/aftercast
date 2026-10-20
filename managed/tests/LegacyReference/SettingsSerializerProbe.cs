using System.Windows.Forms;
using System.Xml;
using Advanced_Combat_Tracker;

namespace Bxivact.LegacyReference;

// Compile the observed XML signatures against original ACT metadata, including its misspelled finalizer.
public static class SettingsSerializerProbe
{
    private static void Check(bool condition)
    {
        if (!condition) throw new Exception("Original-reference settings serializer probe failed");
    }

    public static string Run()
    {
        using var check = new CheckBox { Name = "check", Checked = true };
        using var duplicate = new CheckBox { Name = "duplicate" };
        using var text = new TextBox { Name = "text", Text = "A&B <log>" };
        using var combo = new ComboBox { Name = "different-control-name", DropDownStyle = ComboBoxStyle.DropDownList };
        combo.Items.AddRange(["English", "Japanese", "English"]);
        combo.SelectedIndex = 0;
        var serializer = new SettingsSerializer(new object());
        serializer.AddControlSetting("check", check);
        serializer.AddControlSetting("check", duplicate);
        serializer.AddControlSetting("text", text);
        serializer.AddControlSetting("combo", combo);
        using var output = new StringWriter();
        using (var writer = new XmlTextWriter(output))
        {
            writer.WriteStartElement("Config");
            writer.WriteStartElement("SettingsSerializer");
            serializer.ExportToXml(writer);
            writer.WriteEndElement();
            writer.WriteEndElement();
        }
        check.Checked = false;
        text.Text = "changed";
        combo.SelectedIndex = 1;
        using (var reader = new XmlTextReader(new StringReader(output.ToString())))
        {
            Check(reader.ReadToFollowing("SettingsSerializer"));
            Check(serializer.ImportFromXml(reader) == 0);
        }
        serializer.FinializeComboBoxes();
        Check(check.Checked && !duplicate.Checked && text.Text == "A&B <log>");
        Check(combo.DropDownStyle == ComboBoxStyle.DropDownList && combo.SelectedIndex == 2);

        using (var reader = new XmlTextReader(new StringReader("<Envelope><SettingsSerializer><CheckBox Name='check' Value='invalid'/><TextBox Name='missing' Value='x'/><Unknown/><TextBox Name='text' Value='after'/></SettingsSerializer><After/></Envelope>")))
        {
            Check(reader.ReadToFollowing("SettingsSerializer"));
            Check(serializer.ImportFromXml(reader) == 2 && reader.Name == "After");
            Check(text.Text == "after");
        }
        using var unsupported = new Label();
        try
        {
            serializer.AddControlSetting("unsupported", unsupported);
            throw new Exception("Unsupported control registration succeeded");
        }
        catch (ArgumentException) { }
        serializer.Dispose();
        return text.Text;
    }
}
