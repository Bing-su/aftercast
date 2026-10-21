using System.Diagnostics;
using System.Windows.Forms;

namespace Advanced_Combat_Tracker;

public delegate void ClipboardEventDelegate(ClipboardEventArgs args);
public class ClipboardEventArgs(string clipContent, bool copyLocal) : EventArgs
{
    public string ClipContent { get; set; } = clipContent;
    public bool CopyLocal { get; set; } = copyLocal;
    public string CallerName => new StackTrace().GetFrame(3)!.GetMethod()!.Name;
}

public partial class FormActMain
{
    public event ClipboardEventDelegate? BeforeClipboardSet;
    public void SendToClipboard(string text, bool copyLocal)
    {
        if (!OperatingSystem.IsWindows()) throw new PlatformNotSupportedException("The headless host has no system clipboard");
        var info = new ClipboardEventArgs(text, copyLocal);
        foreach (var callback in BeforeClipboardSet?.GetInvocationList() ?? [])
            try { ((ClipboardEventDelegate)callback)(info); }
            catch (Exception error) { WriteExceptionLog(error, "BeforeClipboardSet"); }
        // With no remote Clipboard Sharer connection, ACT copies locally even when CopyLocal is false.
        if (InvokeRequired) Invoke((Action)(() => CopyToWindowsClipboard(info.ClipContent)));
        else CopyToWindowsClipboard(info.ClipContent);
    }

    private void CopyToWindowsClipboard(string text)
    {
        try { System.Windows.Forms.Clipboard.SetDataObject(text, true); }
        catch (Exception error)
        {
            WriteExceptionLog(error, "Method 1 failed.");
            try
            {
                Thread.Sleep(100);
                using var box = new TextBox { Text = text };
                box.SelectAll();
                box.Copy();
            }
            catch (Exception fallback)
            {
                WriteExceptionLog(fallback, "Method 2 failed.");
                try { Thread.Sleep(100); System.Windows.Forms.Clipboard.SetDataObject(text, false); }
                catch (Exception final) { WriteExceptionLog(final, "Method 3 failed."); }
            }
        }
    }
}
