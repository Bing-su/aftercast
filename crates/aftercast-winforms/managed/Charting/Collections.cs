using System.Collections.ObjectModel;

namespace System.Windows.Forms.DataVisualization.Charting;

public abstract class ChartElementCollection<T> : Collection<T> where T : ChartElement { }

// Preserve ordered Collection<T>.Add and named lookup used by compiled designer code.
public abstract class ChartNamedElementCollection<T> : ChartElementCollection<T> where T : ChartNamedElement
{
    protected abstract string NamePrefix { get; }
    public T this[string name] => this.FirstOrDefault(item => item.Name == name)
        ?? throw new ArgumentException("Chart element name was not found", nameof(name));

    private void Attach(T item, int replacing = -1)
    {
        ArgumentNullException.ThrowIfNull(item);
        if (item.NameAvailable is not null) throw new ArgumentException("Chart element already belongs to a collection", nameof(item));
        bool Available(string name) => this.Where((_, index) => index != replacing).All(existing => existing.Name != name);
        if (item.Name.Length == 0)
        {
            var suffix = 1;
            while (!Available(NamePrefix + suffix)) suffix++;
            item.Name = NamePrefix + suffix;
        }
        if (!Available(item.Name)) throw new ArgumentException("Chart element name already exists", nameof(item));
        // Check live names, so renames cannot introduce duplicates or leave a stale lookup index.
        item.NameAvailable = name => this.All(existing => ReferenceEquals(existing, item) || existing.Name != name);
    }
    protected override void InsertItem(int index, T item)
    {
        Attach(item);
        base.InsertItem(index, item);
    }
    protected override void SetItem(int index, T item)
    {
        var previous = this[index];
        if (ReferenceEquals(previous, item)) return;
        Attach(item, index);
        previous.NameAvailable = null;
        base.SetItem(index, item);
    }
    protected override void RemoveItem(int index)
    {
        this[index].NameAvailable = null;
        base.RemoveItem(index);
    }
    protected override void ClearItems()
    {
        foreach (var item in this) item.NameAvailable = null;
        base.ClearItems();
    }
}

public class SeriesCollection : ChartNamedElementCollection<Series> { protected override string NamePrefix => "Series"; }
public class ChartAreaCollection : ChartNamedElementCollection<ChartArea> { protected override string NamePrefix => "ChartArea"; }
public class LegendCollection : ChartNamedElementCollection<Legend> { protected override string NamePrefix => "Legend"; }
