using System.Reflection;

// Test-only injection into the genuine writer's StreamWriter, never into a user path.
internal sealed class FaultingStream(Stream original, bool writeFailure) : Stream
{
    internal static void Install(Delegate action, bool writeFailure)
    {
        var writer = FindWriter(action.Target!, 0);
        var stream = typeof(StreamWriter).GetField("_stream", BindingFlags.Instance | BindingFlags.NonPublic)
            ?? throw new InvalidOperationException("The .NET 10 StreamWriter injection seam changed");
        stream.SetValue(writer, new FaultingStream(writer.BaseStream, writeFailure));
    }
    private static StreamWriter FindWriter(object closure, int depth)
    {
        if (depth > 3) throw new InvalidOperationException("Vendor writer closure shape changed");
        var values = closure.GetType().GetFields(BindingFlags.Instance | BindingFlags.Public | BindingFlags.NonPublic)
            .Select(field => field.GetValue(closure)).ToArray();
        if (values.OfType<StreamWriter>().FirstOrDefault() is { } writer) return writer;
        var parent = values.FirstOrDefault(value => value?.GetType().Name.Contains("DisplayClass", StringComparison.Ordinal) == true)
            ?? throw new InvalidOperationException("Missing vendor output writer closure");
        return FindWriter(parent, depth + 1);
    }
    public override void Flush()
    {
        original.Flush();
        if (!writeFailure) throw new IOException("Synthetic flush failure after underlying flush; commit is uncertain to caller");
    }
    public override void Write(byte[] buffer, int offset, int count)
    {
        if (writeFailure) throw new IOException("Synthetic write failure before underlying write");
        original.Write(buffer, offset, count);
    }
    public override void Write(ReadOnlySpan<byte> buffer)
    {
        if (writeFailure) throw new IOException("Synthetic write failure before underlying write");
        original.Write(buffer);
    }
    protected override void Dispose(bool disposing) { if (disposing) original.Dispose(); base.Dispose(disposing); }
    public override bool CanRead => original.CanRead;
    public override bool CanSeek => original.CanSeek;
    public override bool CanWrite => original.CanWrite;
    public override long Length => original.Length;
    public override long Position { get => original.Position; set => original.Position = value; }
    public override int Read(byte[] buffer, int offset, int count) => original.Read(buffer, offset, count);
    public override long Seek(long offset, SeekOrigin origin) => original.Seek(offset, origin);
    public override void SetLength(long value) => original.SetLength(value);
}
