using System.Windows.Forms;

namespace Bxivact.LegacyReference;

public static class ButtonProbe
{
    private sealed class TestButton : Button
    {
        // Exercise the original protected event raiser through the replacement Control base.
        public void Raise() => OnClick(EventArgs.Empty);
    }
    public static int Run()
    {
        using var box = new CheckBox { Text = "Capture" };
        ButtonBase styled = box;
        styled.UseVisualStyleBackColor = false;
        var changes = 0;
        EventHandler changed = (_, _) => changes++;
        box.CheckedChanged += changed;
        box.Checked = true;
        box.Checked = true;
        box.CheckedChanged -= changed;
        if (!box.Checked || changes != 1 || styled.UseVisualStyleBackColor)
            throw new Exception("Original checkbox contract failed");
        using var button = new TestButton { Text = "Reset", UseVisualStyleBackColor = true };
        Control control = button;
        var clicks = 0;
        EventHandler click = (_, _) => clicks++;
        control.Click += click;
        button.Raise();
        control.Click -= click;
        button.Raise();
        return changes + clicks;
    }
}
