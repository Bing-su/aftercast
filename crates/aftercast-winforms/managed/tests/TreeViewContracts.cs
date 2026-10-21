using System.Windows.Forms;

internal static class TreeViewContracts
{
    internal static void Run()
    {
        using var first = new TreeView();
        using var second = new TreeView { PathSeparator = "/" };
        var parent = first.Nodes.Add("Data Correction");
        var child = parent.Nodes.Add("EQ2 English Settings");
        if (child.Parent != parent || child.TreeView != first || child.FullPath != "Data Correction\\EQ2 English Settings")
            throw new Exception("Tree node ownership or path mismatch");
        try { second.Nodes.Add(parent); throw new Exception("Owned node added twice"); }
        catch (ArgumentException) { }
        parent.Remove();
        if (first.Nodes.Count != 0 || child.TreeView is not null) throw new Exception("Subtree removal retained its old tree");
        try { _ = child.FullPath; throw new Exception("Detached path unexpectedly succeeded"); }
        catch (InvalidOperationException) { }
        second.Nodes.Add(parent);
        if (child.FullPath != "Data Correction/EQ2 English Settings") throw new Exception("Reattached path mismatch");
        second.Nodes.Clear();
        try { child.Nodes.Add(parent); throw new Exception("Tree cycle accepted"); }
        catch (ArgumentException) { }
        Console.WriteLine("Independent tree-node paths, subtree ownership, reattachment and cycle rejection passed.");
    }
}
