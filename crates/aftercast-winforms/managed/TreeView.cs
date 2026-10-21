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
    public string FullPath => TreeView is null ? throw new InvalidOperationException("Node is not in a TreeView") :
        Parent is null ? Text : Parent.FullPath + TreeView.PathSeparator + Text;
    public void Remove() => Owner?.Remove(this);
}

public class TreeNodeCollection : IEnumerable
{
    private readonly List<TreeNode> nodes = new();
    private readonly TreeView? tree;
    internal TreeNodeCollection(TreeNode? parent, TreeView? tree) { Parent = parent; this.tree = tree; }
    internal TreeNode? Parent { get; }
    internal TreeView? Tree => tree ?? Parent?.TreeView;
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
