using System.Windows.Forms;

internal static class ButtonContracts
{
    private sealed class TestButton : Button
    {
        // Explicit calls verify the protected contract without simulating native interaction.
        internal void Raise(EventArgs e) => OnClick(e);
    }
    private static void Check(bool condition)
    {
        if (!condition) throw new Exception("Button contract check failed");
    }
    public static void Run()
    {
        Check(typeof(ButtonBase).IsAbstract && typeof(ButtonBase).BaseType == typeof(Control));
        Check(typeof(Button).BaseType == typeof(ButtonBase) && typeof(CheckBox).BaseType == typeof(ButtonBase));
        using var box = new CheckBox { Text = "Capture", Name = "capture" };
        Check(!box.Checked && box.UseVisualStyleBackColor);
        box.UseVisualStyleBackColor = false;
        Check(!box.UseVisualStyleBackColor && box.Text == "Capture" && box.Name == "capture");
        var changes = new List<bool>();
        var thread = Environment.CurrentManagedThreadId;
        EventHandler changed = (sender, e) =>
        {
            Check(ReferenceEquals(sender, box) && ReferenceEquals(e, EventArgs.Empty));
            Check(Environment.CurrentManagedThreadId == thread);
            changes.Add(box.Checked);
        };
        box.CheckedChanged += changed;
        box.Checked = false;
        box.Checked = true;
        box.Checked = true;
        box.Checked = false;
        Check(changes.SequenceEqual(new[] { true, false }));
        box.CheckedChanged -= changed;
        box.Checked = true;
        Check(changes.Count == 2);
        box.CheckedChanged += (_, _) => { if (box.Checked) box.Checked = false; };
        box.Checked = false;
        box.Checked = true;
        Check(!box.Checked);
        using var failure = new CheckBox();
        failure.CheckedChanged += (_, _) => throw new InvalidOperationException("handler failure");
        try { failure.Checked = true; throw new Exception("Handler failure swallowed"); }
        catch (InvalidOperationException error) { Check(error.Message == "handler failure" && failure.Checked); }

        using var button = new TestButton();
        Check(button.UseVisualStyleBackColor);
        var clicks = 0;
        var args = new EventArgs();
        EventHandler click = (sender, e) =>
        {
            Check(ReferenceEquals(sender, button) && ReferenceEquals(e, args));
            Check(Environment.CurrentManagedThreadId == thread);
            clicks++;
        };
        Control control = button;
        control.Click += click;
        button.Raise(args);
        control.Click -= click;
        button.Raise(args);
        Check(clicks == 1);
        button.Dispose();
        box.Dispose();
        Check(button.IsDisposed && box.IsDisposed);
        Console.WriteLine("Independent button inheritance, metadata, checkbox transitions and explicit click delivery passed.");
    }
}
