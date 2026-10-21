using System.Collections.Concurrent;
using System.Globalization;
using System.Reflection;

// Observe the parser's sink before the UI's 100-message limit can discard a later exception.
internal sealed class OriginalParserMessageCapture : IDisposable
{
    private readonly object settings;
    private readonly PropertyInfo property;
    private readonly Action<DateTime, string> original;
    private readonly ConcurrentQueue<string> messages;

    internal OriginalParserMessageCapture(object settings, string[] initial)
    {
        if (initial.Length >= 100) throw new InvalidOperationException("Parser startup messages reached the UI limit before capture attached");
        this.settings = settings;
        messages = new(initial);
        property = settings.GetType().GetProperty("ProcessException")
            ?? throw new InvalidOperationException("Original parser diagnostic callback was not found");
        original = (Action<DateTime, string>?)property.GetValue(settings)
            ?? throw new InvalidOperationException("Original parser diagnostic callback was not initialized");
        property.SetValue(settings, (Action<DateTime, string>)Capture);
    }

    private void Capture(DateTime time, string text)
    {
        messages.Enqueue("[" + time.ToString("HH:mm:ss.fff", CultureInfo.InvariantCulture) + "] " + text);
        original(time, text);
    }

    internal string[] Snapshot() => messages.ToArray();
    public void Dispose() => property.SetValue(settings, original);
}
