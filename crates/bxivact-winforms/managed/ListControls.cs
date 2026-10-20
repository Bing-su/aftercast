using System.Collections;
using System.ComponentModel;

namespace System.Windows.Forms;

public enum ComboBoxStyle { Simple, DropDown, DropDownList }

// Keep the observed unbound selection model; data binding and native selection are outside this slice.
public abstract class ListControl : Control
{
    protected readonly List<object> Values = [];
    protected int CurrentIndex = -1;
    public abstract int SelectedIndex { get; set; }
    public bool FormattingEnabled { get; set; }
    protected bool SelectIndex(int value)
    {
        if (value < -1 || value >= Values.Count) throw new ArgumentOutOfRangeException(nameof(value));
        if (CurrentIndex == value) return false;
        CurrentIndex = value;
        base.Text = value < 0 ? "" : Values[value].ToString() ?? "";
        return true;
    }
}

public class ComboBox : ListControl
{
    private ComboBoxStyle style = ComboBoxStyle.DropDown;
    public ComboBox() { Items = new(this); }
    public ObjectCollection Items { get; }
    public ComboBoxStyle DropDownStyle
    {
        get => style;
        set
        {
            if (!Enum.IsDefined(value)) throw new InvalidEnumArgumentException(nameof(value), (int)value, typeof(ComboBoxStyle));
            style = value;
        }
    }
    public override int SelectedIndex
    {
        get => CurrentIndex;
        set { if (SelectIndex(value)) OnSelectedIndexChanged(EventArgs.Empty); }
    }
    public object? SelectedItem
    {
        get => CurrentIndex < 0 ? null : Values[CurrentIndex];
        set
        {
            var index = value is null ? -1 : Values.IndexOf(value);
            if (value is null || index >= 0) SelectedIndex = index;
        }
    }
    public event EventHandler? SelectedIndexChanged;
    protected virtual void OnSelectedIndexChanged(EventArgs e) => SelectedIndexChanged?.Invoke(this, e);
    public class ObjectCollection : IEnumerable
    {
        private readonly ComboBox owner;
        public ObjectCollection(ComboBox owner) { this.owner = owner ?? throw new ArgumentNullException(nameof(owner)); }
        public int Count => owner.Values.Count;
        public object this[int index] => owner.Values[index];
        public int Add(object item)
        {
            ArgumentNullException.ThrowIfNull(item);
            owner.Values.Add(item);
            return Count - 1;
        }
        public void AddRange(object[] items)
        {
            ArgumentNullException.ThrowIfNull(items);
            foreach (var item in items) Add(item);
        }
        public bool Contains(object item) => owner.Values.Contains(item);
        public void Clear()
        {
            owner.Values.Clear();
            owner.CurrentIndex = -1;
        }
        public IEnumerator GetEnumerator() => owner.Values.GetEnumerator();
    }
}

public class ListBox : ListControl
{
    public ListBox() { Items = new(this); }
    public ObjectCollection Items { get; }
    public bool ScrollAlwaysVisible { get; set; }
    public override int SelectedIndex
    {
        get => CurrentIndex;
        set { if (SelectIndex(value)) OnSelectedIndexChanged(EventArgs.Empty); }
    }
    public event EventHandler? SelectedIndexChanged;
    protected virtual void OnSelectedIndexChanged(EventArgs e) => SelectedIndexChanged?.Invoke(this, e);
    public class ObjectCollection : IEnumerable
    {
        private readonly ListBox owner;
        public ObjectCollection(ListBox owner) { this.owner = owner ?? throw new ArgumentNullException(nameof(owner)); }
        public int Count => owner.Values.Count;
        public object this[int index] => owner.Values[index];
        public int Add(object item)
        {
            ArgumentNullException.ThrowIfNull(item);
            owner.Values.Add(item);
            return Count - 1;
        }
        public bool Contains(object item) => owner.Values.Contains(item);
        public void Clear()
        {
            owner.Values.Clear();
            owner.SelectedIndex = -1;
        }
        public IEnumerator GetEnumerator() => owner.Values.GetEnumerator();
    }
}
