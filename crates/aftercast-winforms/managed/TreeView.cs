using System.Collections;

namespace System.Windows.Forms;

// Preserve parser option-node ownership and paths, e.g. Data Correction\EQ2 English Settings, without rendering.
public class TreeView : Control
{
    public TreeView() { Nodes = new(null, this); }
    public TreeNodeCollection Nodes { get; }
    public string PathSeparator { get; set; } = "\\";
}

public class TreeNode : MarshalByRefObject
{
    private string text = "";
    internal TreeNodeCollection? Owner;
    public TreeNode() : this("") { }
    public TreeNode(string text) { Text = text; Nodes = new(this, null); }
    public string Text { get => text; set => text = value ?? ""; }
    public TreeNodeCollection Nodes { get; }
    public TreeNode? Parent => Owner?.Parent;
    public TreeView? TreeView => Owner?.Tree;
    public int Index => Owner?.IndexOf(this) ?? -1;
    public string FullPath
    {
        get
        {
            var tree = TreeView ?? throw new InvalidOperationException("Node is not in a TreeView");
            if (Parent is null) return Text;
            // Build once from live names, e.g. moving a subtree or changing PathSeparator needs no cache invalidation.
            var parts = new List<string>();
            for (TreeNode? node = this; node is not null; node = node.Parent) parts.Add(node.Text);
            parts.Reverse();
            return string.Join(tree.PathSeparator, parts);
        }
    }
    public void Remove() => Owner?.Remove(this);
}

public class TreeNodeCollection : IEnumerable
{
    private readonly List<TreeNode> nodes = new();
    private readonly TreeView? tree;
    internal TreeNodeCollection(TreeNode? parent, TreeView? tree) { Parent = parent; this.tree = tree; }
    internal TreeNode? Parent { get; }
    internal TreeView? Tree
    {
        get
        {
            var collection = this;
            while (collection.Parent?.Owner is { } owner) collection = owner;
            return collection.tree;
        }
    }
    public int Count => nodes.Count;
    public TreeNode this[int index] => nodes[index];
    public TreeNode Add(string text) { var node = new TreeNode(text); Add(node); return node; }
    public int Add(TreeNode node)
    {
        ArgumentNullException.ThrowIfNull(node);
        if (node.Owner is not null) throw new ArgumentException("Node already belongs to a collection", nameof(node));
        for (var parent = Parent; parent is not null; parent = parent.Parent)
            if (ReferenceEquals(parent, node)) throw new ArgumentException("A node cannot contain itself", nameof(node));
        nodes.Add(node);
        node.Owner = this;
        return nodes.Count - 1;
    }
    public int IndexOf(TreeNode node) => nodes.IndexOf(node);
    public void Remove(TreeNode node) { if (nodes.Remove(node)) node.Owner = null; }
    public void Clear() { foreach (var node in nodes) node.Owner = null; nodes.Clear(); }
    public IEnumerator GetEnumerator() => nodes.GetEnumerator();
}
