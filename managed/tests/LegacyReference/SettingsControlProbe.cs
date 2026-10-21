using System.Windows.Forms;

namespace Aftercast.LegacyReference;

public static class SettingsControlProbe
{
    // Verify designer/member signatures without invoking dialogs or fabricating user responses.
    public static int Run()
    {
        using var group = new GroupBox { TabStop = false };
        using var combo = new ComboBox { DropDownStyle = ComboBoxStyle.DropDownList, FormattingEnabled = true };
        combo.Items.AddRange(new object[] { "one", "two" });
        var changes = 0;
        combo.SelectedIndexChanged += (_, _) => changes++;
        ((ListControl)combo).SelectedIndex = 1;
        if (!combo.Items.Contains("two") || (string)combo.SelectedItem! != "two") throw new Exception("Combo lookup failed");
        combo.SelectedItem = "one";
        using var list = new ListBox { ScrollAlwaysVisible = true };
        list.Items.Add("x");
        foreach (var item in list.Items) if ((string)item != "x") throw new Exception("List enumeration failed");
        list.Items.Clear();
        using var text = new TextBox { Multiline = true, ReadOnly = true, Text = "value" };
        using var link = new LinkLabel { TabStop = true };
        LinkLabelLinkClickedEventHandler handler = (_, _) => { };
        link.LinkClicked += handler;
        link.LinkClicked -= handler;
        using var dialog = new FolderBrowserDialog { SelectedPath = "/tmp" };
        try { ((CommonDialog)dialog).ShowDialog(); throw new Exception("Dialog succeeded"); }
        catch (PlatformNotSupportedException) { }
        return changes;
    }
}
