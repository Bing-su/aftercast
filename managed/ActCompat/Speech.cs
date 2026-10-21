using System.Text;

namespace Advanced_Combat_Tracker;

public partial class FormActMain
{
    private readonly object speechGate = new();
    private readonly Queue<byte[]> speech = new();
    private bool speechEnabled;
    private int speechDepth;
    internal void ConfigureSpeech(bool enabled)
    {
        lock (speechGate) { speechEnabled = enabled; speech.Clear(); }
    }
    internal byte[] DrainSpeech()
    {
        lock (speechGate) return speech.TryDequeue(out var text) ? text : Array.Empty<byte>();
    }
    public void TTS(string SpeechText)
    {
        try
        {
            if (Interlocked.Increment(ref speechDepth) > 100) throw new InvalidOperationException("Too many recursive TTS calls");
            PlayTtsMethod(SpeechText);
        }
        // Original ACT reports TTS failures through its diagnostic log rather than propagating them to the plugin.
        catch (Exception error) { WriteExceptionLog(error, "TTS"); }
        finally { Interlocked.Decrement(ref speechDepth); }
    }
    private void QueueSpeech(string SpeechText)
    {
            ArgumentNullException.ThrowIfNull(SpeechText);
            if (SpeechText.Length > 4096) throw new ArgumentException("TTS text exceeds its bounded size", nameof(SpeechText));
            var text = new UTF8Encoding(false, true).GetBytes(SpeechText);
            if (string.IsNullOrWhiteSpace(SpeechText) || text.Length > 4096 || SpeechText.Any(c => char.IsControl(c) && c is not ('\n' or '\r' or '\t')))
                throw new ArgumentException("TTS requires nonempty plain text of at most 4096 UTF-8 bytes", nameof(SpeechText));
            lock (speechGate)
            {
                if (IsActClosing) throw new InvalidOperationException("TTS is closing");
                if (!speechEnabled) throw new NotSupportedException("Configure a Rust TTS backend before requesting speech");
                if (speech.Count == 32) throw new InvalidOperationException("TTS request queue is full");
                // Copy before returning: plugins may invoke TTS on workers while Rust drains requests on its owner thread.
                speech.Enqueue(text);
            }
    }
}
