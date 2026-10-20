using System.ComponentModel;
using FormsTimer = System.Windows.Forms.Timer;

namespace Bxivact.LegacyReference;

public static class TimerProbe
{
    private sealed class TestTimer : FormsTimer
    {
        // Bind the protected original signature without depending on a native message loop.
        public void Raise() => OnTick(EventArgs.Empty);
    }
    public static int Run()
    {
        using var container = new Container();
        var timer = new FormsTimer(container) { Enabled = true, Interval = 1000 };
        EventHandler handler = (_, _) => throw new Exception("Headless timer scheduled a callback");
        timer.Tick += handler;
        if (timer.Container != container || !timer.Enabled || timer.Interval != 1000)
            throw new Exception("Original timer signatures did not bind");
        timer.Stop();
        timer.Start();
        timer.Tick -= handler;
        container.Dispose();
        if (timer.Enabled || timer.Container is not null) throw new Exception("Timer lifetime failed");
        using var manual = new TestTimer();
        var count = 0;
        manual.Tick += (_, _) => count++;
        manual.Raise();
        return count;
    }
}
