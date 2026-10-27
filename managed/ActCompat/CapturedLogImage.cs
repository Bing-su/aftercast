using System.Security.Cryptography;

namespace Advanced_Combat_Tracker;

// Private, temporary byte images avoid re-opening mutable source paths during replay.
// Hash every chunk before exposing any of its bytes, including when a range stops before EOF.
internal sealed class CapturedLogImage : IDisposable
{
    internal const int ChunkBytes = 64 * 1024;
    private readonly FileStream file;
    private readonly byte[][] chunks;
    internal long Length { get; }
    internal string Digest { get; }
    private int disposed, opened;
    private CapturedLogImage(FileStream file, long length, byte[][] chunks, string digest)
    { this.file = file; Length = length; this.chunks = chunks; Digest = digest; }

    internal static async Task<CapturedLogImage> CopyAsync(FileStream source, long length, string scratch,
        CancellationToken cancellation, Action<int> progress, Func<FileStream, Stream>? writeAdapter = null)
    {
        var path = Path.Combine(scratch, ".aftercast-source-" + Guid.NewGuid().ToString("N") + ".tmp");
        var options = new FileStreamOptions { Mode = FileMode.CreateNew, Access = FileAccess.ReadWrite, Share = FileShare.None,
            BufferSize = 1, Options = FileOptions.Asynchronous | FileOptions.DeleteOnClose | FileOptions.RandomAccess };
        if (OperatingSystem.IsLinux()) options.UnixCreateMode = UnixFileMode.UserRead | UnixFileMode.UserWrite;
        var target = new FileStream(path, options);
        Stream? adapter = null; CapturedLogImage? result = null; var failures = new List<Exception>();
        try
        {
            // The internal adapter seam permits deterministic disk/flush failures in tests; production passes none.
            adapter = writeAdapter?.Invoke(target);
            var sink = adapter ?? target;
            // An owned new scratch file, never a source. Linux process exit releases the unlinked inode too.
            if (OperatingSystem.IsLinux()) File.Delete(path);
            using var hash = IncrementalHash.CreateHash(HashAlgorithmName.SHA256);
            var buffer = new byte[ChunkBytes]; var chunks = new List<byte[]>(); long copied = 0;
            while (copied < length)
            {
                cancellation.ThrowIfCancellationRequested();
                var count = (int)Math.Min(buffer.Length, length - copied); var read = 0;
                while (read < count)
                {
                    var received = await source.ReadAsync(buffer.AsMemory(read, count - read), cancellation).ConfigureAwait(false);
                    if (received == 0) throw new EndOfStreamException("Selected source ended before its admitted length");
                    read += received;
                }
                await sink.WriteAsync(buffer.AsMemory(0, count), cancellation).ConfigureAwait(false);
                chunks.Add(SHA256.HashData(buffer.AsSpan(0, count))); hash.AppendData(buffer, 0, count);
                copied += count; progress(count);
            }
            await sink.FlushAsync(cancellation).ConfigureAwait(false); cancellation.ThrowIfCancellationRequested();
            result = new(target, length, chunks.ToArray(), Convert.ToHexStringLower(hash.GetHashAndReset()));
        }
        catch (Exception error) { failures.Add(error); }
        if (adapter is not null && !ReferenceEquals(adapter, target))
            try { adapter.Dispose(); } catch (Exception error) { failures.Add(error); }
        if (failures.Count != 0)
        {
            try { target.Dispose(); } catch (Exception error) { failures.Add(error); }
            if (failures.Count == 1) System.Runtime.ExceptionServices.ExceptionDispatchInfo.Capture(failures[0]).Throw();
            throw new AggregateException("Captured image write/cleanup failed", failures);
        }
        return result!;
    }
    internal Stream OpenReader()
    {
        ObjectDisposedException.ThrowIf(Volatile.Read(ref disposed) != 0, this);
        if (Interlocked.Exchange(ref opened, 1) != 0) throw new InvalidOperationException("Captured image is single-reader");
        return new VerifiedReader(this);
    }
    public void Dispose() { if (Interlocked.Exchange(ref disposed, 1) == 0) file.Dispose(); }

    private sealed class VerifiedReader(CapturedLogImage image) : Stream
    {
        private readonly byte[] buffer = new byte[ChunkBytes];
        private long position, loaded = -1;
        private int valid;
        private bool closed;
        public override bool CanRead => !closed;
        public override bool CanSeek => !closed;
        public override bool CanWrite => false;
        public override long Length { get { RequireOpen(); return image.Length; } }
        public override long Position { get { RequireOpen(); return position; } set => Seek(value, SeekOrigin.Begin); }
        private void RequireOpen() => ObjectDisposedException.ThrowIf(closed || Volatile.Read(ref image.disposed) != 0, this);
        private int Needed(long chunk) => (int)Math.Min(ChunkBytes, image.Length - chunk * ChunkBytes);
        private void Verified(long chunk, int count)
        {
            if (!CryptographicOperations.FixedTimeEquals(SHA256.HashData(buffer.AsSpan(0, count)), image.chunks[checked((int)chunk)]))
                throw new InvalidDataException("Captured source chunk integrity check failed; partial replay may have been retained");
            loaded = chunk; valid = count;
        }
        private int Copy(Span<byte> destination)
        {
            var offset = (int)(position % ChunkBytes); var count = Math.Min(destination.Length, valid - offset);
            buffer.AsSpan(offset, count).CopyTo(destination); position += count; return count;
        }
        public override int Read(byte[] destination, int offset, int count) => Read(destination.AsSpan(offset, count));
        public override int Read(Span<byte> destination)
        {
            RequireOpen(); if (position == image.Length || destination.Length == 0) return 0;
            var chunk = position / ChunkBytes;
            if (loaded != chunk)
            {
                var count = Needed(chunk); var read = 0;
                while (read < count)
                {
                    var received = RandomAccess.Read(image.file.SafeFileHandle, buffer.AsSpan(read, count - read), chunk * ChunkBytes + read);
                    if (received == 0) throw new EndOfStreamException("Captured image was truncated"); read += received;
                }
                Verified(chunk, count);
            }
            return Copy(destination);
        }
        public override async ValueTask<int> ReadAsync(Memory<byte> destination, CancellationToken cancellationToken = default)
        {
            RequireOpen(); cancellationToken.ThrowIfCancellationRequested();
            if (position == image.Length || destination.Length == 0) return 0;
            var chunk = position / ChunkBytes;
            if (loaded != chunk)
            {
                var count = Needed(chunk); var read = 0;
                while (read < count)
                {
                    var received = await RandomAccess.ReadAsync(image.file.SafeFileHandle, buffer.AsMemory(read, count - read),
                        chunk * ChunkBytes + read, cancellationToken).ConfigureAwait(false);
                    if (received == 0) throw new EndOfStreamException("Captured image was truncated"); read += received;
                }
                Verified(chunk, count);
            }
            cancellationToken.ThrowIfCancellationRequested(); return Copy(destination.Span);
        }
        public override Task<int> ReadAsync(byte[] destination, int offset, int count, CancellationToken cancellationToken) => ReadAsync(destination.AsMemory(offset, count), cancellationToken).AsTask();
        public override long Seek(long offset, SeekOrigin origin)
        {
            RequireOpen(); var next = checked(origin switch { SeekOrigin.Begin => offset, SeekOrigin.Current => position + offset,
                SeekOrigin.End => image.Length + offset, _ => throw new ArgumentOutOfRangeException(nameof(origin)) });
            if (next < 0 || next > image.Length) throw new IOException("Captured image seek is out of bounds"); return position = next;
        }
        public override void Flush() { RequireOpen(); }
        public override void SetLength(long value) => throw new NotSupportedException("Captured images are read-only");
        public override void Write(byte[] buffer, int offset, int count) => throw new NotSupportedException("Captured images are read-only");
        protected override void Dispose(bool disposing)
        { if (closed) return; closed = true; try { if (disposing) image.Dispose(); } finally { base.Dispose(disposing); } }
    }
}
