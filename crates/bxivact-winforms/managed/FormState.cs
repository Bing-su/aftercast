using System.ComponentModel;

namespace System.Windows.Forms;

public enum FormWindowState { Normal = 0, Minimized = 1, Maximized = 2 }

public partial class Form
{
    private FormWindowState windowState;
    // Preserve requested state as metadata; no native resize, visibility change or event is synthesized.
    public FormWindowState WindowState
    {
        get => windowState;
        set
        {
            if (!Enum.IsDefined(value))
                throw new InvalidEnumArgumentException(nameof(value), (int)value, typeof(FormWindowState));
            windowState = value;
        }
    }
}
