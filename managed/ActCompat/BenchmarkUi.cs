using System.Runtime.InteropServices;
using System.Windows.Forms;

namespace Advanced_Combat_Tracker;

public class ListViewNoFlicker : ListView
{
    private int updateCount;
    public bool CustomGridLines { get; set; } = true;
    public ListViewNoFlicker()
    {
        if (!OperatingSystem.IsWindows()) throw new PlatformNotSupportedException("ACT benchmark lists require native WinForms");
        ConfigureNativeBuffering();
    }
    private void ConfigureNativeBuffering() => DoubleBuffered = true;
    public new void BeginUpdate() { base.BeginUpdate(); updateCount++; }
    public new void EndUpdate() { base.EndUpdate(); updateCount--; }
    public void FlushUpdate() { while (updateCount > 0) EndUpdate(); }
    public void SetExStyles()
    {
        CustomGridLines = GridLines;
        // shortcut: native grid/header rendering substitutes ACT's themed owner drawing; add theme parity with ACT UI options.
        var styles = SendMessage(Handle, 0x1037, IntPtr.Zero, IntPtr.Zero);
        SendMessage(Handle, 0x1036, IntPtr.Zero, new IntPtr(styles.ToInt64() | 0x18000));
    }
    [DllImport("user32.dll", CharSet = CharSet.Auto)]
    private static extern IntPtr SendMessage(IntPtr handle, int message, IntPtr wparam, IntPtr lparam);
}

public static class ThreadInvokes
{
    public static void RichTextBoxAppendDateTimeLine(Form parent, RichTextBox target, string text)
    {
        if (target.InvokeRequired)
            parent.Invoke((Action)(() => RichTextBoxAppendDateTimeLine(parent, target, text)));
        else
        {
            target.AppendText($"\n[{DateTime.Now.ToLongTimeString()}] {text}");
            target.ScrollToCaret();
        }
    }
}

public partial class FormActMain
{
    private int listViewWidthOffset = int.MinValue;
    public string LastLogLine { get; set; } = string.Empty;
    public void WriteInfoLog(string message) => Console.Out.WriteLine($"***** {DateTime.Now:s} - {message}");
    public void ResizeLVCols(ListView listview)
    {
        if (!OperatingSystem.IsWindows()) throw new PlatformNotSupportedException("Column sizing requires native WinForms");
        try { ResizeNativeColumns(listview); }
        catch (Exception error) { WriteExceptionLog(error, string.Empty); }
    }
    private void ResizeNativeColumns(ListView listview)
    {
        var weights = new int[listview.Columns.Count];
        var longest = new int[weights.Length];
        foreach (ListViewItem row in listview.Items)
            for (var column = 0; column < weights.Length; column++)
            {
                var length = row.SubItems[column].Text.Length;
                longest[column] = Math.Max(longest[column], length);
                weights[column] += length + listview.Columns[column].Text.Length;
            }
        for (var column = 0; column < weights.Length; column++)
            weights[column] += listview.Items.Count == 0 ? 10 : longest[column] * listview.Items.Count;
        var total = weights.Sum();
        var rows = listview.VirtualMode ? listview.VirtualListSize : listview.Items.Count;
        var scrollbar = rows > 0 && listview.Height < (rows + 1) * listview.Items[0].Bounds.Height
            ? SystemInformation.VerticalScrollBarWidth : 0;
        if (listViewWidthOffset == int.MinValue && scrollbar == 0 && listview.FindForm()?.WindowState != FormWindowState.Minimized)
            listViewWidthOffset = Math.Abs(listview.ClientSize.Width - listview.Width);
        listview.BeginUpdate();
        try
        {
            var used = 0;
            for (var column = 0; column < weights.Length; column++)
            {
                var width = (int)((float)(listview.Width - scrollbar) * ((float)weights[column] / total));
                listview.Columns[column].Width = width;
                used += width;
                if (column == weights.Length - 1)
                    listview.Columns[0].Width += listview.Width - used - (listViewWidthOffset > 0 ? listViewWidthOffset : 6) - scrollbar;
            }
        }
        finally { listview.EndUpdate(); }
    }
}
