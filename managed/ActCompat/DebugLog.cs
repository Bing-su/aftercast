using System.Diagnostics;

namespace Advanced_Combat_Tracker;

public partial class FormActMain
{
    public void WriteDebugLog(string debugMessage)
    {
        // Embedded applications own trace listeners; ordinary ACT debug messages must not become parser errors.
        try { Trace.WriteLine(debugMessage, "ACT"); }
        catch { /* Match ACT's best-effort debug sink: listener failures cannot interrupt combat processing. */ }
    }
}
