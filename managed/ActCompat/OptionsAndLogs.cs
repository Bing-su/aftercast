using System.Drawing;
using System.Windows.Forms;

namespace Advanced_Combat_Tracker;

// Embedded imports have no progress window; the nullable global lets consumers detect the absence of UI.
public class FormImportProgress : Form { }

public partial class FormActMain
{
    public TreeView OptionsTreeView { get; set; } = new();
    public Dictionary<string, List<Control>> OptionsControlSets { get; set; } = new();
}

// Retain the parser's log-type registrations; this does not implement the encounter-log browser UI.
public class FormEncounterLogs : Form
{
    internal FormEncounterLogs(bool isClone) { }
    public Dictionary<int, Color> LogTypeToColorMapping { get; set; } = Enumerable.Range(-1, 12)
        .ToDictionary(type => type, type => type <= 0 ? Color.Gray : Color.Black);
}
