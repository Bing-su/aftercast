using System.ComponentModel;
using System.Drawing;
using System.Windows.Forms;

internal static class FormStateContracts
{
    private static void Check(bool condition)
    {
        if (!condition) throw new Exception("Form state contract check failed");
    }

    internal static void Run()
    {
        Check((int)FormWindowState.Normal == 0 && (int)FormWindowState.Minimized == 1 && (int)FormWindowState.Maximized == 2);
        using var form = new Form { Size = new Size(100, 200), Location = new Point(3, 4) };
        Check(form.WindowState == FormWindowState.Normal);
        var resized = 0;
        form.Resize += (_, _) => resized++;
        foreach (var state in Enum.GetValues<FormWindowState>())
        {
            form.WindowState = state;
            Check(form.WindowState == state);
        }
        foreach (var value in new[] { -1, 3, int.MaxValue })
        {
            try { form.WindowState = (FormWindowState)value; throw new Exception("Invalid form state accepted"); }
            catch (InvalidEnumArgumentException error) { Check(error.ParamName == "value"); }
            Check(form.WindowState == FormWindowState.Maximized);
        }
        Check(resized == 0 && form.Size == new Size(100, 200) && form.Location == new Point(3, 4) && form.Visible);
        Console.WriteLine("Headless form state, enum validation and unchanged geometry checks passed.");
    }
}
