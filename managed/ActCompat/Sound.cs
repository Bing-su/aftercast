using System.Text;

namespace Advanced_Combat_Tracker;

public partial class FormActMain
{
    public delegate void PlaySoundDelegate(string WavFilePath, int VolumePercent);
    public delegate void PlayTtsDelegate(string TtsString);
    public PlaySoundDelegate PlaySoundMethod = null!;
    public PlayTtsDelegate PlayTtsMethod = null!;
    private readonly object soundGate = new();
    private readonly Queue<Tuple<byte[], int>> sounds = new();
    private bool soundEnabled;
    private int soundDepth;
    internal int SoundVolume = 100, TtsVolume = 100;
    internal string TimerStartSound = "none", TimerWarningSound = "none", TimerExpireSound = "none";
    internal void InitializeAudio() { PlaySoundMethod = QueueSound; PlayTtsMethod = QueueSpeech; }
    internal void ConfigureSound(bool enabled) { lock (soundGate) { soundEnabled = enabled; sounds.Clear(); } }
    internal Tuple<byte[], int> DrainSound()
    {
        lock (soundGate) return sounds.TryDequeue(out var sound) ? sound : Tuple.Create(Array.Empty<byte>(), 0);
    }
    public string MakeAbsolutePathString(string FilePath, string ReferenceFolderPath) =>
        Path.IsPathRooted(FilePath) ? FilePath : Path.GetFullPath(Path.Combine(string.IsNullOrWhiteSpace(ReferenceFolderPath) ? AppContext.BaseDirectory : ReferenceFolderPath, FilePath));
    public void PlaySound(string pszSound)
    {
        try
        {
            if (Interlocked.Increment(ref soundDepth) > 100) throw new InvalidOperationException("Too many recursive sound calls");
            ArgumentException.ThrowIfNullOrWhiteSpace(pszSound);
            var source = MakeAbsolutePathString(pszSound, "");
            if (!File.Exists(source)) source = new Uri(source).AbsoluteUri;
            PlaySoundMethod(source, source.EndsWith("sapi.wav", StringComparison.Ordinal) ? TtsVolume : SoundVolume);
        }
        // ACT logs and rethrows sound failures, unlike its TTS failure boundary.
        catch (Exception error) { WriteExceptionLog(error, "PlaySound: " + pszSound); throw; }
        finally { Interlocked.Decrement(ref soundDepth); }
    }
    internal void Beep() => PlaySoundMethod("beep", SoundVolume);
    private void QueueSound(string source, int volume)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(source);
        if (source.Length > 4096) throw new ArgumentException("Sound source exceeds its bounded size");
        var bytes = new UTF8Encoding(false, true).GetBytes(source);
        if (bytes.Length > 4096 || source.Any(char.IsControl) || volume is < 0 or > 100) throw new ArgumentException("Invalid sound source or volume");
        lock (soundGate)
        {
            if (IsActClosing || !soundEnabled) throw new InvalidOperationException("Configure a Rust sound player before requesting audio");
            if (sounds.Count == 32) throw new InvalidOperationException("Sound queue is full");
            sounds.Enqueue(Tuple.Create(bytes, volume));
        }
    }
}
