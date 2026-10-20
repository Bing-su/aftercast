using System.ComponentModel;
using FormsTimer = System.Windows.Forms.Timer;

internal static class TimerContracts
{
    private sealed class TestTimer : FormsTimer
    {
        internal int EnabledWrites;
        public override bool Enabled
        {
            get => base.Enabled;
            set { EnabledWrites++; base.Enabled = value; }
        }
        // Exercise event dispatch explicitly; the headless adapter does not schedule callbacks.
        internal void Raise(EventArgs args) => OnTick(args);
    }
    private static void Check(bool condition)
    {
        if (!condition) throw new Exception("Timer contract check failed");
    }
    public static void Run()
    {
        try { _ = new FormsTimer(null!); throw new Exception("Null timer container accepted"); }
        catch (ArgumentNullException) { }
        using var container = new Container();
        var timer = new FormsTimer(container);
        Check(timer.Container == container && container.Components.Count == 1);
        Check(!timer.Enabled && timer.Interval == 100);
        foreach (var invalid in new[] { 0, -1, int.MinValue })
        {
            try { timer.Interval = invalid; throw new Exception("Invalid timer interval accepted"); }
            catch (ArgumentOutOfRangeException) { Check(timer.Interval == 100); }
        }
        timer.Interval = int.MaxValue;
        Check(timer.Interval == int.MaxValue);
        using var unexpectedTick = new ManualResetEventSlim();
        timer.Tick += (_, _) => unexpectedTick.Set();
        timer.Interval = 1;
        timer.Start();
        Check(timer.Enabled && !unexpectedTick.Wait(50));
        timer.Stop();
        Check(!timer.Enabled);
        timer.Enabled = true;
        var disposed = false;
        timer.Disposed += (_, _) => { Check(!timer.Enabled); disposed = true; };
        container.Dispose();
        Check(disposed && !timer.Enabled && timer.Container is null);

        using var manual = new TestTimer();
        manual.Start();
        manual.Stop();
        Check(manual.EnabledWrites == 2 && !manual.Enabled);
        var calls = 0;
        var args = new EventArgs();
        var thread = Environment.CurrentManagedThreadId;
        EventHandler handler = (sender, received) =>
        {
            Check(ReferenceEquals(sender, manual) && ReferenceEquals(received, args));
            Check(Environment.CurrentManagedThreadId == thread);
            calls++;
        };
        manual.Tick += handler;
        manual.Raise(args);
        manual.Tick -= handler;
        manual.Raise(args);
        Check(calls == 1);
        Console.WriteLine("Independent timer configuration, container disposal and explicit event dispatch passed; automatic ticks are unsupported.");
    }
}
