using System.Windows.Forms;

internal static class UiChecks
{
    internal static Control CreateControl()
    {
        var control = new Control();
        // Native Invoke needs an owner-thread handle, unlike the Linux context-backed model.
        if (OperatingSystem.IsWindows()) _ = control.Handle;
        return control;
    }
}
