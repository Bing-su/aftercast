namespace System.Windows.Forms;

// Preserve the shared button base and designer metadata without drawing visual styles.
public abstract class ButtonBase : Control
{
    public bool UseVisualStyleBackColor { get; set; } = true;
}

// shortcut: dialog-button contracts are absent, add them when dialog hosting is supported.
public class Button : ButtonBase { }

// shortcut: only programmatic boolean state is modeled, add automatic toggling with input support.
public class CheckBox : ButtonBase
{
    private bool isChecked;
    public bool Checked
    {
        get => isChecked;
        set
        {
            if (isChecked == value) return;
            // Publish the state before notifying handlers, which may change it again.
            isChecked = value;
            OnCheckedChanged(EventArgs.Empty);
        }
    }
    public event EventHandler? CheckedChanged;
    protected virtual void OnCheckedChanged(EventArgs e) => CheckedChanged?.Invoke(this, e);
    protected override void Dispose(bool disposing) { if (disposing) CheckedChanged = null; base.Dispose(disposing); }
}
