using System.Windows.Forms;

namespace Aftercast.LegacyReference;

public static class TreeViewProbe
{
    public static string Run()
    {
        using var tree = new TreeView();
        var root = tree.Nodes.Add("Data Correction");
        var child = root.Nodes.Add("EQ2 English Settings");
        if (child.FullPath != "Data Correction\\EQ2 English Settings" || child.Parent != root || child.TreeView != tree)
            throw new Exception("Compiled TreeView member contract mismatch");
        child.Remove();
        if (root.Nodes.Count != 0 || child.TreeView is not null) throw new Exception("Compiled TreeNode removal mismatch");
        try { _ = child.FullPath; throw new Exception("Detached node had a full path"); }
        catch (InvalidOperationException) { }
        return "Tree nodes";
    }
}
