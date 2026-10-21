using System.Collections;
using System.ComponentModel;
using System.Drawing;

namespace System.Windows.Forms;

// Keep ownership, caller-owned dispatch and disposal together without fabricating native windows.
public class Control : Component
{
    private bool disposed;
    private string text = "";
    public string Name { get; set; } = "";
    public virtual string Text { get => text; set => text = value ?? ""; }
    // Keep event wiring usable without synthesizing mouse input or a native window.
    public event EventHandler? Click;
    protected virtual void OnClick(EventArgs e) => Click?.Invoke(this, e);
    public bool IsDisposed => disposed;
    public nint Handle => throw new PlatformNotSupportedException("Linux WinForms native handles are not implemented");
    public void CreateControl() => throw new PlatformNotSupportedException("Linux WinForms rendering is not implemented");
    protected override void Dispose(bool disposing)
    {
        if (disposed || Disposing) return;
        Disposing = true;
        var errors = new List<Exception>();
        try
        {
            if (disposing)
            {
                Parent?.Controls.Remove(this);
                // Attempt every child even when a plugin's disposal callback fails.
                foreach (Control child in Controls.Cast<Control>().ToArray())
                    try { child.Dispose(); } catch (Exception error) { errors.Add(error); }
                Controls.Clear();
            }
            disposed = true;
            try { base.Dispose(disposing); } catch (Exception error) { errors.Add(error); }
        }
        finally { Disposing = false; }
        if (errors.Count != 0) throw new AggregateException("Control disposal failed", errors);
    }

    private Point location;
    private Size size;
    private bool visible = true;
    private bool enabled = true;
    private int tabIndex;
    private readonly int ownerThread = Environment.CurrentManagedThreadId;
    private readonly SynchronizationContext? ownerContext = CaptureContext();
    private Control? parent;
    private ControlCollection? controls;
    public bool Disposing { get; private set; }
    public bool InvokeRequired => Environment.CurrentManagedThreadId != ownerThread;
    public object? Invoke(Delegate method)
    {
        ArgumentNullException.ThrowIfNull(method);
        ObjectDisposedException.ThrowIf(IsDisposed, this);
        if (!InvokeRequired) return InvokeOnOwner(method);
        if (ownerContext is null) throw new PlatformNotSupportedException("Linux cross-thread UI dispatch requires a caller-owned message pump");
        object? result = null;
        System.Runtime.ExceptionServices.ExceptionDispatchInfo? failure = null;
        var dispatched = false;
        // Send delegates scheduling to the caller's pump; never substitute a pool thread or inline callback.
        ownerContext.Send(_ =>
        {
            try { result = InvokeOnOwner(method); }
            catch (Exception error) { failure = System.Runtime.ExceptionServices.ExceptionDispatchInfo.Capture(error); }
            finally { dispatched = true; }
        }, null);
        if (!dispatched) throw new PlatformNotSupportedException("The caller-owned UI context did not dispatch synchronously");
        failure?.Throw();
        return result;
    }
    private object? InvokeOnOwner(Delegate method)
    {
        if (InvokeRequired) throw new PlatformNotSupportedException("The caller-owned UI context did not dispatch to the control's owner thread");
        ObjectDisposedException.ThrowIf(IsDisposed, this);
        try { return method.DynamicInvoke(); }
        catch (System.Reflection.TargetInvocationException error) when (error.InnerException is not null)
        { System.Runtime.ExceptionServices.ExceptionDispatchInfo.Capture(error.InnerException).Throw(); throw; }
    }
    private static SynchronizationContext? CaptureContext()
    {
        var context = SynchronizationContext.Current;
        // The base context sends inline, so it cannot provide cross-thread UI dispatch.
        return context?.GetType() == typeof(SynchronizationContext) ? null : context;
    }
    public Control? Parent
    {
        get => parent;
        set
        {
            if (ReferenceEquals(parent, value)) return;
            if (value is null) parent?.Controls.Remove(this);
            else value.Controls.Add(this);
        }
    }
    public ControlCollection Controls => controls ??= new(this);
    public Point Location { get => location; set => location = value; }
    public Size Size
    {
        get => size;
        set
        {
            var next = new Size(Math.Max(0, value.Width), Math.Max(0, value.Height));
            if (next == size) return;
            size = next;
            OnResize(EventArgs.Empty);
        }
    }
    public int Width { get => Size.Width; set => Size = new(value, Height); }
    public int Height { get => Size.Height; set => Size = new(Width, value); }
    public virtual bool AutoSize { get; set; }
    public int TabIndex
    {
        get => tabIndex;
        set { ArgumentOutOfRangeException.ThrowIfNegative(value); tabIndex = value; }
    }
    public bool TabStop { get; set; } = true;
    public bool Visible { get => visible && (Parent?.Visible ?? true); set => visible = value; }
    public bool Enabled { get => enabled && (Parent?.Enabled ?? true); set => enabled = value; }
    public DockStyle Dock { get; set; }
    public AnchorStyles Anchor { get; set; } = AnchorStyles.Top | AnchorStyles.Left;
    public Padding Margin { get; set; } = new(3);
    public event EventHandler? Resize;
    protected virtual void OnResize(EventArgs e) => Resize?.Invoke(this, e);
    // shortcut: no layout events or engine exist, implement batching when layout dispatch is supported.
    public void SuspendLayout() { }
    public void ResumeLayout() => ResumeLayout(true);
    public void ResumeLayout(bool performLayout) { }
    public void PerformLayout() { }

    public class ControlCollection : IEnumerable
    {
        private readonly Control owner;
        private readonly List<Control> children = [];
        // Only the canonical Controls property may own children; detached collections would lose cleanup ownership.
        internal ControlCollection(Control owner) { this.owner = owner; }
        public int Count => children.Count;
        public Control this[int index] => children[index];
        public virtual Control? this[string key] => string.IsNullOrEmpty(key) ? null :
            children.Find(child => string.Equals(child.Name, key, StringComparison.OrdinalIgnoreCase));
        public virtual void Add(Control? value)
        {
            if (value is null) return;
            ObjectDisposedException.ThrowIf(owner.IsDisposed || owner.Disposing, owner);
            ObjectDisposedException.ThrowIf(value.IsDisposed || value.Disposing, value);
            for (Control? ancestor = owner; ancestor is not null; ancestor = ancestor.Parent)
                if (ReferenceEquals(ancestor, value)) throw new ArgumentException("Circular control parent", nameof(value));
            if (value is Form) throw new ArgumentException("Top-level forms cannot be child controls", nameof(value));
            if (ReferenceEquals(value.Parent, owner)) return;
            value.Parent?.Controls.Remove(value);
            children.Add(value);
            value.SetParent(owner);
        }
        public virtual void AddRange(Control[] values)
        {
            ArgumentNullException.ThrowIfNull(values);
            foreach (var value in values) Add(value);
        }
        public virtual void Remove(Control? value)
        {
            var index = children.FindIndex(child => ReferenceEquals(child, value));
            if (index < 0) return;
            children.RemoveAt(index);
            value!.SetParent(null);
        }
        public virtual void Clear()
        {
            foreach (var child in children) child.SetParent(null);
            children.Clear();
        }
        public IEnumerator GetEnumerator() => children.GetEnumerator();
    }
    // Update the backing field without re-entering the public reparenting operation.
    private void SetParent(Control? value) => parent = value;
}
