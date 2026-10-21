using System.Collections;
using System.ComponentModel;

namespace System.Windows.Forms;

public enum View { LargeIcon, Details, SmallIcon, List, Tile }

// Retain performance rows without creating native list controls or dispatching UI events.
public class ListView : Control
{
    private View view;
    public ListView()
    {
        Columns = new ColumnHeaderCollection(this);
        Items = new ListViewItemCollection(this);
    }
    public ColumnHeaderCollection Columns { get; }
    public ListViewItemCollection Items { get; }
    public SelectedListViewItemCollection SelectedItems { get; } = new();
    public bool FullRowSelect { get; set; }
    public bool HideSelection { get; set; } = true;
    public bool UseCompatibleStateImageBehavior { get; set; } = true;
    public View View
    {
        get => view;
        set
        {
            if (!Enum.IsDefined(value)) throw new InvalidEnumArgumentException(nameof(value), (int)value, typeof(View));
            view = value;
        }
    }

    public class ColumnHeaderCollection : IEnumerable
    {
        private readonly ListView owner;
        public ColumnHeaderCollection(ListView owner) { this.owner = owner ?? throw new ArgumentNullException(nameof(owner)); }
        private readonly List<ColumnHeader> columns = [];
        public int Count => columns.Count;
        public ColumnHeader this[int index] => columns[index];
        public int Add(ColumnHeader value)
        {
            ArgumentNullException.ThrowIfNull(value);
            if (value.ListView is not null) throw new ArgumentException("Column already belongs to a ListView", nameof(value));
            columns.Add(value);
            value.ListView = owner;
            return columns.Count - 1;
        }
        public void AddRange(ColumnHeader[] values)
        {
            ArgumentNullException.ThrowIfNull(values);
            foreach (var value in values) Add(value);
        }
        public void Remove(ColumnHeader value)
        {
            if (columns.Remove(value)) value.ListView = null;
        }
        public void Clear()
        {
            foreach (var column in columns) column.ListView = null;
            columns.Clear();
        }
        public IEnumerator GetEnumerator() => columns.GetEnumerator();
    }

    public class ListViewItemCollection : IEnumerable
    {
        private readonly ListView owner;
        public ListViewItemCollection(ListView owner) { this.owner = owner ?? throw new ArgumentNullException(nameof(owner)); }
        private readonly List<ListViewItem> items = [];
        public int Count => items.Count;
        public ListViewItem this[int index] => items[index];
        // Read current names on every lookup; plugins assign Name after adding a row.
        public ListViewItem? this[string key] => string.IsNullOrEmpty(key) ? null :
            items.Find(item => string.Equals(item.Name, key, StringComparison.OrdinalIgnoreCase));
        public bool ContainsKey(string key) => this[key] is not null;
        public ListViewItem Add(ListViewItem value)
        {
            ArgumentNullException.ThrowIfNull(value);
            if (value.ListView is not null) throw new ArgumentException("Item already belongs to a ListView", nameof(value));
            items.Add(value);
            value.ListView = owner;
            return value;
        }
        public void Remove(ListViewItem value)
        {
            if (items.Remove(value)) value.ListView = null;
        }
        public void Clear()
        {
            foreach (var item in items) item.ListView = null;
            items.Clear();
        }
        public IEnumerator GetEnumerator() => items.GetEnumerator();
    }

    public class SelectedListViewItemCollection : IEnumerable
    {
        // shortcut: no native handle or selection API exists, add selection state when either is supported.
        internal SelectedListViewItemCollection() { }
        public int Count => 0;
        public ListViewItem this[int index] => throw new ArgumentOutOfRangeException(nameof(index));
        public IEnumerator GetEnumerator() => Array.Empty<ListViewItem>().GetEnumerator();
    }
}

public class ColumnHeader : Component
{
    private string text = "ColumnHeader";
    public string Text { get => text; set => text = value ?? ""; }
    public int Width { get; set; } = 60;
    public ListView? ListView { get; internal set; }
}

public class ListViewItem
{
    private string name = "";
    public ListViewItem() : this(Array.Empty<string>()) { }
    public ListViewItem(string[] items) { SubItems = new ListViewSubItemCollection(items); }
    public string Name { get => name; set => name = value ?? ""; }
    public string Text { get => SubItems[0].Text; set => SubItems[0].Text = value; }
    public ListView? ListView { get; internal set; }
    public ListViewSubItemCollection SubItems { get; }

    public class ListViewSubItem
    {
        private string text = "";
        public string Text { get => text; set => text = value ?? ""; }
    }

    public class ListViewSubItemCollection : IEnumerable
    {
        private readonly List<ListViewSubItem> items = [];
        internal ListViewSubItemCollection(string[]? values)
        {
            // Subitem zero is the item's text, including when the input array is empty.
            if (values is not { Length: > 0 }) items.Add(new ListViewSubItem());
            else foreach (var value in values) items.Add(new ListViewSubItem { Text = value });
        }
        public int Count => items.Count;
        public ListViewSubItem this[int index] => items[index];
        public IEnumerator GetEnumerator() => items.GetEnumerator();
    }
}
