using System.Windows.Forms;

internal static class ListControlContracts
{
    private static void Check(bool condition) { if (!condition) throw new Exception("Selection contract check failed"); }
    public static void Run()
    {
        using var combo = new ComboBox();
        Check(combo.SelectedIndex == -1 && combo.SelectedItem is null && combo.DropDownStyle == ComboBoxStyle.DropDown);
        combo.Items.AddRange(["A", "B"]);
        var changes = 0;
        combo.SelectedIndexChanged += (_, _) => changes++;
        combo.SelectedItem = "B";
        Check(combo.SelectedIndex == 1 && combo.Text == "B" && changes == 1);
        combo.SelectedIndex = 1;
        combo.SelectedItem = "missing";
        Check(changes == 1 && combo.SelectedIndex == 1);
        try { combo.SelectedIndex = 5; throw new Exception("Bad index accepted"); }
        catch (ArgumentOutOfRangeException) { Check(combo.SelectedIndex == 1); }
        combo.Items.Clear();
        Check(combo.SelectedItem is null && combo.SelectedIndex == -1 && changes == 1);
        try { combo.Items.Add(null!); throw new Exception("Null item accepted"); }
        catch (ArgumentNullException) { }
        using var list = new ListBox();
        list.Items.Add("one");
        list.SelectedIndex = 0;
        list.Items.Clear();
        Check(list.Items.Count == 0 && list.SelectedIndex == -1);
        Console.WriteLine("Unbound list selection, lookup, validation and clearing passed.");
    }
}
