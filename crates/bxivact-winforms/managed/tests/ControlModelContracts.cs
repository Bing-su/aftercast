using System.Drawing;
using System.Windows.Forms;
using System.Windows.Forms.DataVisualization.Charting;

internal static class ControlModelContracts
{
    private static void Check(bool condition) { if (!condition) throw new Exception("Control model check failed"); }
    public static void Run()
    {
        ControlInvokeContracts.Run();
        using var root = new UserControl();
        using var other = new Panel();
        Check(!root.InvokeRequired && (int)root.Invoke((Func<int>)(() => 42))! == 42);
        var crossThreadRejected = false;
        var worker = new Thread(() =>
        {
            Check(root.InvokeRequired);
            try { root.Invoke((Action)(() => throw new Exception("Wrong thread executed UI callback"))); }
            catch (PlatformNotSupportedException) { crossThreadRejected = true; }
        });
        worker.Start();
        worker.Join();
        Check(crossThreadRejected);
        var child = new Label();
        root.Controls.Add(child);
        root.Controls.Add(child);
        root.Controls.Add(null);
        Check(root.Controls.Count == 1 && child.Parent == root);
        other.Controls.Add(child);
        Check(root.Controls.Count == 0 && other.Controls.Count == 1 && child.Parent == other);
        child.Parent = root;
        Check(other.Controls.Count == 0 && child.Parent == root);
        try { child.Controls.Add(root); throw new Exception("Parent cycle accepted"); }
        catch (ArgumentException) { Check(root.Parent is null && child.Parent == root); }
        root.Visible = false;
        root.Enabled = false;
        Check(!child.Visible && !child.Enabled);
        root.Visible = true;
        root.Enabled = true;
        Check(child.Visible && child.Enabled);
        var resized = 0;
        root.Resize += (_, _) => resized++;
        root.SuspendLayout();
        root.Location = new Point(3, 4);
        root.Size = new Size(100, 200);
        root.Width = 100;
        root.Height = 201;
        root.ResumeLayout(false);
        root.PerformLayout();
        Check(resized == 2 && root.Width == 100 && root.Height == 201 && root.Location.X == 3);
        using var chart = new Chart();
        chart.Size = new Size(50, 60);
        Check(((Control)chart).Width == 50 && chart.Height == 60);
        var first = new Label();
        var last = new Label();
        root.Controls.AddRange([first, last]);
        first.Disposed += (_, _) => throw new Exception("child cleanup failure");
        root.Disposed += (_, _) => Check(root.Disposing && root.IsDisposed);
        try { root.Dispose(); throw new Exception("Disposal error swallowed"); }
        catch (AggregateException error) { Check(error.ToString().Contains("child cleanup failure")); }
        Check(root.IsDisposed && !root.Disposing && child.IsDisposed && first.IsDisposed && last.IsDisposed);
        Check(root.Controls.Count == 0 && child.Parent is null);
        root.Dispose();
        Console.WriteLine("Control geometry, reparenting, cycle rejection and complete child cleanup passed.");
    }
}
