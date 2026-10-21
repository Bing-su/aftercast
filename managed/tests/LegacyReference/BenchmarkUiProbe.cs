using System.Runtime.CompilerServices;
using System.Windows.Forms;
using Advanced_Combat_Tracker;

namespace Aftercast.LegacyReference;

public static class BenchmarkUiProbe
{
    public static string Run()
    {
        var root = (FormActMain)RuntimeHelpers.GetUninitializedObject(typeof(FormActMain));
        GC.SuppressFinalize(root);
        using var form = new Form();
        using var list = new ListViewNoFlicker { Width = 400, Height = 1000, View = View.Details };
        form.Controls.Add(list);
        if (!list.CustomGridLines) throw new Exception("List grid default mismatch");
        list.SetExStyles();
        list.Columns.Add("A"); list.Columns.Add("BBBB");
        root.ResizeLVCols(list);
        if (list.Columns[0].Width != 194 || list.Columns[1].Width != 200)
            throw new Exception("Empty column proportions differ");
        list.Items.Add(new ListViewItem(new[] { "12", "123456" }));
        root.ResizeLVCols(list);
        if (list.Columns[0].Width != 90 || list.Columns[1].Width != 304)
            throw new Exception($"Populated column proportions differ: {list.Columns[0].Width},{list.Columns[1].Width}");
        list.BeginUpdate(); list.BeginUpdate(); list.EndUpdate(); list.FlushUpdate();
        using var log = new RichTextBox();
        form.Controls.Add(log);
        ThreadInvokes.RichTextBoxAppendDateTimeLine(form, log, "Fixture message");
        if (!log.Text.StartsWith("\n[") || !log.Text.EndsWith("] Fixture message"))
            throw new Exception("Timestamped UI log differs");
        return "Benchmark UI";
    }
}
