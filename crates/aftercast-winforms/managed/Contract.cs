namespace System.Windows.Forms;

// Preserve the hierarchy required by compiled plugins, e.g. TabPage -> Panel -> ScrollableControl.
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
public partial class Form : ContainerControl { }
