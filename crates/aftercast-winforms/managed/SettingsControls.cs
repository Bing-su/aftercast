using System.ComponentModel;

namespace System.Windows.Forms;

// Retain designer settings without constructing native edit controls or group borders.
public class GroupBox : Control
{
    public GroupBox() { TabStop = false; }
}

public abstract class TextBoxBase : Control
{
    public virtual bool Multiline { get; set; }
    public bool ReadOnly { get; set; }
}

public class TextBox : TextBoxBase { }

public delegate void LinkLabelLinkClickedEventHandler(object? sender, LinkLabelLinkClickedEventArgs e);

public class LinkLabelLinkClickedEventArgs : EventArgs
{
    public LinkLabelLinkClickedEventArgs(LinkLabel.Link? link) : this(link, MouseButtons.Left) { }
    public LinkLabelLinkClickedEventArgs(LinkLabel.Link? link, MouseButtons button)
    { Link = link; Button = button; }
    public LinkLabel.Link? Link { get; }
    public MouseButtons Button { get; }
}

public class LinkLabel : Label
{
    // Explicit event delivery preserves handler binding without opening links or simulating input.
    public event LinkLabelLinkClickedEventHandler? LinkClicked;
    protected virtual void OnLinkClicked(LinkLabelLinkClickedEventArgs e) => LinkClicked?.Invoke(this, e);
    // shortcut: only link identity is modeled; add ranges and ownership when Links is required.
    public class Link { }
}

[Flags]
public enum MouseButtons
{
    None = 0, Left = 0x00100000, Right = 0x00200000, Middle = 0x00400000,
    XButton1 = 0x00800000, XButton2 = 0x01000000
}

// Keep the .NET Framework values because existing plugin IL embeds these numbers.
public enum DialogResult { None = 0, OK = 1, Cancel = 2, Abort = 3, Retry = 4, Ignore = 5, Yes = 6, No = 7 }
public enum MessageBoxButtons { OK = 0, OKCancel = 1, AbortRetryIgnore = 2, YesNoCancel = 3, YesNo = 4, RetryCancel = 5 }

public abstract class CommonDialog : Component
{
    // A missing modal host must not fabricate a user's confirmation or cancellation.
    public DialogResult ShowDialog() => throw new PlatformNotSupportedException("Linux WinForms modal dialogs are not implemented");
}

public sealed class FolderBrowserDialog : CommonDialog
{
    private string selectedPath = "";
    public string SelectedPath { get => selectedPath; set => selectedPath = value ?? ""; }
}

public static class Clipboard
{
    // Reject unavailable integration instead of reporting a successful copy that lost the text.
    public static void SetText(string text) => throw new PlatformNotSupportedException("Linux WinForms clipboard access is not implemented");
}

public class MessageBox
{
    private MessageBox() { }
    public static DialogResult Show(string? text, string? caption, MessageBoxButtons buttons) =>
        throw new PlatformNotSupportedException("Linux WinForms message boxes are not implemented");
}
