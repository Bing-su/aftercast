using System.Windows.Forms;

internal static class ListViewContracts
{
    private static void Check(bool condition)
    {
        if (!condition) throw new Exception("ListView contract check failed");
    }
    private static void Reject(Action action)
    {
        try { action(); }
        catch (ArgumentException) { return; }
        throw new Exception("Invalid ListView operation succeeded");
    }
    public static void Run()
    {
        using var list = new ListView();
        using var other = new ListView();
        Reject(() => _ = new ListView.ColumnHeaderCollection(null!));
        Reject(() => _ = new ListView.ListViewItemCollection(null!));
        Check(list is Control && list.HideSelection && !list.FullRowSelect);
        Check(list.UseCompatibleStateImageBehavior && list.View == View.LargeIcon);
        list.View = View.Details;
        Reject(() => list.View = (View)99);
        Check(list.View == View.Details);
        var column = new ColumnHeader { Text = "Network", Width = 120 };
        list.Columns.AddRange([column, new ColumnHeader { Text = "Count" }]);
        Check(list.Columns.Count == 2 && ReferenceEquals(list.Columns[0], column));
        Check(column.ListView == list && column.Width == 120);
        Reject(() => other.Columns.Add(column));
        Reject(() => list.Columns.Add(column));
        Reject(() => list.Columns.Add(null!));
        Reject(() => list.Columns.AddRange(null!));
        Reject(() => _ = list.Columns[-1]);
        list.Columns.Remove(column);
        other.Columns.Add(column);
        other.Columns.Clear();
        Check(column.ListView is null);

        var row = new ListViewItem(["Network", "0", "0"]);
        Check(ReferenceEquals(list.Items.Add(row), row) && row.ListView == list);
        row.Name = "capture";
        Check(list.Items.ContainsKey("CAPTURE") && ReferenceEquals(list.Items["Capture"], row));
        Check(list.Items[""] is null && list.Items["missing"] is null);
        row.Name = "renamed";
        Check(!list.Items.ContainsKey("capture") && list.Items.ContainsKey("renamed"));
        row.SubItems[0].Text = "Receive";
        Check(row.Text == "Receive");
        row.Text = "Send";
        row.SubItems[1].Text = "42";
        Check(row.SubItems[0].Text == "Send" && row.SubItems[1].Text == "42" && row.SubItems.Count == 3);
        Reject(() => list.Items.Add(row));
        Reject(() => other.Items.Add(row));
        Reject(() => list.Items.Add(null!));
        Reject(() => _ = list.Items[1]);
        Reject(() => _ = row.SubItems[3]);
        var duplicate = new ListViewItem([]) { Name = "RENAMED" };
        list.Items.Add(duplicate);
        Check(ReferenceEquals(list.Items["renamed"], row));
        Check(list.Items.Cast<ListViewItem>().SequenceEqual(new[] { row, duplicate }));
        list.Items.Remove(row);
        Check(ReferenceEquals(list.Items["renamed"], duplicate) && row.ListView is null);
        other.Items.Add(row);
        other.Items.Clear();
        list.Items.Clear();
        Check(row.ListView is null && duplicate.ListView is null && list.Items.Count == 0);
        foreach (var item in new[] { new ListViewItem(), new ListViewItem([]), new ListViewItem(null!) })
        {
            Check(item.SubItems.Count == 1 && item.Text == "");
            item.Text = null!;
            item.Name = null!;
            Check(item.SubItems[0].Text == "" && item.Name == "");
        }
        Check(list.SelectedItems.Count == 0 && !list.SelectedItems.Cast<ListViewItem>().Any());
        Reject(() => _ = list.SelectedItems[0]);
        try { _ = list.Handle; throw new Exception("ListView created a native handle"); }
        catch (PlatformNotSupportedException) { }
        Console.WriteLine("Independent ListView columns, row ownership, live key lookup and subitems passed.");
    }
}
