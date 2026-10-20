using System.ComponentModel;

namespace System.Windows.Forms;

// Store status and disposal state without fabricating native windows or a message loop.
public partial class Control : Component
{
    private bool disposed;
    private string text = "";
    public string Name { get; set; } = "";
    public virtual string Text { get => text; set => text = value ?? ""; }
    // Keep event wiring usable without synthesizing mouse input or a native window.
    public event EventHandler? Click;
    protected virtual void OnClick(EventArgs e) => Click?.Invoke(this, e);
    public bool IsDisposed => disposed;
    public nint Handle => throw new PlatformNotSupportedException("Linux WinForms native handles are not implemented");
    public void CreateControl() => throw new PlatformNotSupportedException("Linux WinForms rendering is not implemented");
    protected override void Dispose(bool disposing)
    {
        if (disposed || Disposing) return;
        Disposing = true;
        var errors = new List<Exception>();
        try
        {
            if (disposing)
            {
                Parent?.Controls.Remove(this);
                // Attempt every child even when a plugin's disposal callback fails.
                foreach (Control child in Controls.Cast<Control>().ToArray())
                    try { child.Dispose(); } catch (Exception error) { errors.Add(error); }
                Controls.Clear();
            }
            disposed = true;
            try { base.Dispose(disposing); } catch (Exception error) { errors.Add(error); }
        }
        finally { Disposing = false; }
        if (errors.Count != 0) throw new AggregateException("Control disposal failed", errors);
    }
}

public class ScrollableControl : Control { }
public class Panel : ScrollableControl { }
public class TabPage : Panel { }
public class Label : Control { }
public class ContainerControl : ScrollableControl
{
    public System.Drawing.SizeF AutoScaleDimensions { get; set; }
    public AutoScaleMode AutoScaleMode { get; set; } = AutoScaleMode.Inherit;
}
// Resolve settings base types without claiming a rendered settings panel.
public class UserControl : ContainerControl
{
    public AutoSizeMode AutoSizeMode { get; set; } = AutoSizeMode.GrowOnly;
}
public class Form : ContainerControl { }
