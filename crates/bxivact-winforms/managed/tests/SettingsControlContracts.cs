using System.ComponentModel;
using System.Windows.Forms;

internal static class SettingsControlContracts
{
    private sealed class TestLinkLabel : LinkLabel
    {
        // Verify protected dispatch directly without inventing native mouse input.
        internal void Raise(LinkLabelLinkClickedEventArgs e) => OnLinkClicked(e);
    }

    private static void Check(bool condition)
    {
        if (!condition) throw new Exception("Settings control contract check failed");
    }

    private static void Unsupported(Action action, string diagnostic)
    {
        try { action(); throw new Exception("Unsupported settings interaction unexpectedly succeeded"); }
        catch (PlatformNotSupportedException error) { Check(error.Message.Contains(diagnostic)); }
    }

    public static void Run()
    {
        Check(typeof(GroupBox).BaseType == typeof(Control));
        Check(typeof(TextBoxBase).IsAbstract && typeof(TextBoxBase).BaseType == typeof(Control));
        Check(typeof(TextBox).BaseType == typeof(TextBoxBase) && typeof(LinkLabel).BaseType == typeof(Label));
        using var group = new GroupBox();
        Check(!group.TabStop);
        group.TabStop = true;
        using var text = new TextBox();
        Check(!text.Multiline && !text.ReadOnly);
        text.Multiline = true;
        text.ReadOnly = true;
        text.Text = "Capture settings";
        Check(text.Multiline && text.ReadOnly && text.Text == "Capture settings" && group.TabStop);

        using var label = new TestLinkLabel();
        var link = new LinkLabel.Link();
        var args = new LinkLabelLinkClickedEventArgs(link);
        Check(ReferenceEquals(args.Link, link) && args.Button == MouseButtons.Left);
        var alternate = new LinkLabelLinkClickedEventArgs(null, MouseButtons.Right);
        Check(alternate.Link is null && alternate.Button == MouseButtons.Right);
        var calls = 0;
        var thread = Environment.CurrentManagedThreadId;
        LinkLabelLinkClickedEventHandler handler = (sender, e) =>
        {
            Check(ReferenceEquals(sender, label) && ReferenceEquals(e, args));
            Check(Environment.CurrentManagedThreadId == thread);
            calls++;
        };
        label.LinkClicked += handler;
        label.Raise(args);
        label.LinkClicked -= handler;
        label.Raise(args);
        Check(calls == 1);
        label.LinkClicked += (_, _) => throw new InvalidOperationException("link handler failure");
        try { label.Raise(args); throw new Exception("Link handler failure swallowed"); }
        catch (InvalidOperationException error) { Check(error.Message == "link handler failure"); }

        Check(typeof(CommonDialog).IsAbstract && typeof(CommonDialog).BaseType == typeof(Component));
        using var folder = new FolderBrowserDialog();
        Check(folder.SelectedPath == "");
        folder.SelectedPath = "/tmp/logs";
        Check(folder.SelectedPath == "/tmp/logs");
        folder.SelectedPath = null!;
        Check(folder.SelectedPath == "");
        Unsupported(() => folder.ShowDialog(), "modal dialogs");
        Unsupported(() => Clipboard.SetText("Log content"), "clipboard");
        Unsupported(() => MessageBox.Show("Restart?", "Settings", MessageBoxButtons.YesNo), "message boxes");
        Check(Enum.GetValues<DialogResult>().Select(value => (int)value).SequenceEqual(Enumerable.Range(0, 8)));
        Check(Enum.GetValues<MessageBoxButtons>().Select(value => (int)value).SequenceEqual(Enumerable.Range(0, 6)));
        Check((int)MouseButtons.Left == 0x00100000 && (int)MouseButtons.XButton2 == 0x01000000);
        Console.WriteLine("Independent settings metadata, link event delivery and unsupported dialog/clipboard boundaries passed.");
    }
}
