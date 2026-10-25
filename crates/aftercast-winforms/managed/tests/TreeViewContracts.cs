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
        const int depth = 2048;
        var leaf = second.Nodes.Add("root");
        var names = new List<string> { "root" };
        for (var index = 1; index < depth; index++)
        {
            leaf = leaf.Nodes.Add("node");
            names.Add("node");
        }
        if (leaf.TreeView != second || leaf.FullPath != string.Join('/', names))
            throw new Exception("Deep tree traversal lost ownership or path segments");
        second.Nodes[0].Text = "renamed";
        second.PathSeparator = "::";
        names[0] = "renamed";
        if (leaf.FullPath != string.Join("::", names)) throw new Exception("Deep paths retained stale names or separators");
        second.Nodes.Clear();
        if (leaf.TreeView is not null) throw new Exception("Detached deep subtree retained its tree");
        Console.WriteLine("Independent tree-node paths, subtree ownership, reattachment and cycle rejection passed.");
    }
}
