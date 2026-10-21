using System.ComponentModel;
using System.Windows.Forms;

namespace Aftercast.LegacyReference;

public static class FormStateProbe
{
    // Compile get/set references against genuine WinForms, then bind them to the Linux metadata model.
    public static int Run()
    {
        using var form = new Form();
        if (form.WindowState != FormWindowState.Normal) throw new Exception("Default form state changed");
        form.WindowState = FormWindowState.Minimized;
        if (form.WindowState != FormWindowState.Minimized) throw new Exception("Minimized form state binding failed");
        form.WindowState = FormWindowState.Maximized;
        try { form.WindowState = (FormWindowState)3; throw new Exception("Invalid form state accepted"); }
        catch (InvalidEnumArgumentException) { }
        return (int)form.WindowState;
    }
}
