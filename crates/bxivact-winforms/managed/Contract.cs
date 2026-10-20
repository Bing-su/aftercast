using System.ComponentModel;

namespace System.Windows.Forms;

// Store status and disposal state without fabricating native windows or a message loop.
public class Control : Component
{
    private bool disposed;
    private string text = "";
    public string Name { get; set; } = "";
    public virtual string Text { get => text; set => text = value ?? ""; }
    public bool IsDisposed => disposed;
    public nint Handle => throw new PlatformNotSupportedException("Linux WinForms native handles are not implemented");
    public void CreateControl() => throw new PlatformNotSupportedException("Linux WinForms rendering is not implemented");
    protected override void Dispose(bool disposing)
    {
        if (disposed) return;
        disposed = true;
        base.Dispose(disposing);
    }
}

public class ScrollableControl : Control { }
public class Panel : ScrollableControl { }
public class TabPage : Panel { }
public class Label : Control { }
public class ContainerControl : ScrollableControl { }
// Resolve settings base types without claiming a rendered settings panel.
public class UserControl : ContainerControl { }
public class Form : ContainerControl { }
