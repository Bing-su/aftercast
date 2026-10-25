using System.Xml;
using Advanced_Combat_Tracker;

internal static class ParentSettingsChecks
{
    private sealed class Settings
    {
        public int Count = 7;
        public string Secret = "unchanged";
        public bool Enabled { get; set; }
    }

    internal static void Run()
    {
        var parent = new Settings();
        using var serializer = new SettingsSerializer(parent);
        serializer.AddIntSetting("Count");
        serializer.AddBooleanSetting("Enabled");
        using var reader = new XmlTextReader(new StringReader("<Settings><Int32 Name='Count' Value='2147483648'/><String Name='Secret' Value='changed'/><Boolean Name='Enabled' Value='true'/></Settings>"));
        reader.MoveToContent();
        if (serializer.ImportFromXml(reader) != 2 || parent.Count != 7 || parent.Secret != "unchanged" || !parent.Enabled)
            throw new Exception("Invalid/unregistered settings must not mutate state or block later settings");
        using var control = new System.Windows.Forms.TextBox();
        serializer.AddControlSetting("text", control);
        if (!serializer.RemoveControlSetting("text") || serializer.RemoveControlSetting("text"))
            throw new Exception("Control removal did not report registration state");
        using var dtd = new XmlTextReader(new StringReader("<!DOCTYPE Settings [<!ENTITY text SYSTEM 'file:///etc/hostname'>]><Settings/>"));
        try { serializer.ImportFromXml(dtd); }
        catch (XmlException) { Console.WriteLine("Parent settings validation and DTD rejection passed."); return; }
        throw new Exception("Settings accepted a DTD");
    }
}
