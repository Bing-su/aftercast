using System.Drawing;
using System.Windows.Forms;

namespace Bxivact.LegacyReference;

public static class ControlModelProbe
{
    // Bind the designer's geometry and ownership signatures against the original reference.
    public static int Run()
    {
        using var page = new UserControl();
        page.SuspendLayout();
        page.AutoScaleDimensions = new SizeF(6, 13);
        page.AutoScaleMode = AutoScaleMode.Font;
        page.AutoSizeMode = AutoSizeMode.GrowAndShrink;
        page.Margin = new Padding(4);
        using var child = new Button { Location = new Point(10, 20), Size = new Size(100, 30), TabIndex = 2, AutoSize = true, Visible = true };
        page.Controls.Add(child);
        page.Resize += (_, _) => child.Width = page.Width;
        page.Width = 200;
        page.ResumeLayout(false);
        page.PerformLayout();
        if (child.Parent != page || child.Height != 30 || child.Location.Y != 20)
            throw new Exception("Control model reference binding failed");
        return child.Width;
    }
}
