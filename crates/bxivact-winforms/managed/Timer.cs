using System.ComponentModel;

namespace System.Windows.Forms;

// Retain designer timer configuration and container lifetime on the caller's thread.
public class Timer : Component
{
    private int interval = 100;
    public Timer() { }
    public Timer(IContainer container)
    {
        ArgumentNullException.ThrowIfNull(container);
        container.Add(this);
    }
    // shortcut: no automatic ticks without a UI pump, add dispatch when the host owns a verified pump.
    public virtual bool Enabled { get; set; }
    public int Interval
    {
        get => interval;
        set
        {
            ArgumentOutOfRangeException.ThrowIfLessThan(value, 1);
            interval = value;
        }
    }
    public event EventHandler? Tick;
    protected virtual void OnTick(EventArgs e) => Tick?.Invoke(this, e);
    public void Start() => Enabled = true;
    public void Stop() => Enabled = false;
    protected override void Dispose(bool disposing)
    {
        if (disposing) Enabled = false;
        base.Dispose(disposing);
    }
}
