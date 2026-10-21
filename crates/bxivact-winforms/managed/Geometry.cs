namespace System.Windows.Forms;

public enum AutoScaleMode { None, Font, Dpi, Inherit }
public enum AutoSizeMode { GrowAndShrink, GrowOnly }
public enum DockStyle { None, Top, Bottom, Left, Right, Fill }
[Flags]
public enum AnchorStyles { None = 0, Top = 1, Bottom = 2, Left = 4, Right = 8 }

public struct Padding
{
    public Padding(int all) : this(all, all, all, all) { }
    public Padding(int left, int top, int right, int bottom)
    { Left = left; Top = top; Right = right; Bottom = bottom; }
    public int Left { get; set; }
    public int Top { get; set; }
    public int Right { get; set; }
    public int Bottom { get; set; }
}
