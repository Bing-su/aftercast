using System.Windows.Forms;

namespace Aftercast.LegacyReference;

public static class ListViewProbe
{
    // Exercise the original nested type signatures, including the performance page's row-update pattern.
    public static string Run()
    {
        using var list = new ListView
        {
            FullRowSelect = true,
            HideSelection = false,
            UseCompatibleStateImageBehavior = false,
            View = View.Details
        };
        list.Columns.AddRange(new[] { new ColumnHeader { Text = "Network", Width = 120 }, new ColumnHeader { Text = "Count" } });
        var row = list.Items.Add(new ListViewItem(new[] { "Receive", "0" }));
        row.Name = "receive";
        if (!list.Items.ContainsKey("RECEIVE")) throw new Exception("Row key lookup failed");
        list.Items["receive"].SubItems[1].Text = "42";
        foreach (ListViewItem item in list.Items)
            if (item.SubItems[1].Text != "42") throw new Exception("Row enumeration failed");
        if (list.SelectedItems.Count != 0 || list.Columns.Count != 2 || list.Columns[0].Width != 120 ||
            !list.FullRowSelect || list.HideSelection || list.UseCompatibleStateImageBehavior || list.View != View.Details)
            throw new Exception("ListView metadata failed");
        try { _ = list.SelectedItems[0]; throw new Exception("Unselected row unexpectedly existed"); }
        catch (ArgumentOutOfRangeException) { }
        return row.SubItems[1].Text;
    }
}
