using System.ComponentModel;
using System.Runtime.InteropServices;
using Microsoft.Win32.SafeHandles;

namespace Advanced_Combat_Tracker;

// Compare open-file identities so equal length/timestamps, renames and hard links cannot impersonate a new file.
internal readonly record struct LogFileIdentity(ulong Volume, ulong Low, ulong High)
{
    internal static LogFileIdentity Read(SafeFileHandle handle)
    {
        if (OperatingSystem.IsLinux())
        {
            // AT_EMPTY_PATH binds statx to this owned descriptor, avoiding a pathname race after opening it.
            if (Statx(handle, "", 0x1000, 0x100, out var info) != 0) throw Failure();
            if ((info.Mask & 0x100) == 0) throw new NotSupportedException("Log filesystem does not supply inode identities");
            return new(((ulong)info.DeviceMajor << 32) | info.DeviceMinor, info.Inode, 0);
        }
        if (OperatingSystem.IsWindows())
        {
            // FileIdInfo includes the full 128-bit ID needed by ReFS as well as the volume serial number.
            if (!GetFileInformationByHandleEx(handle, 18, out WindowsFileId info, 24)) throw Failure();
            if (info.Low == 0 && info.High == 0) throw new NotSupportedException("Log filesystem does not supply file identities");
            return new(info.Volume, info.Low, info.High);
        }
        throw new PlatformNotSupportedException("Log following requires Linux or Windows file identities");
    }

    // Metadata-only proposals never open/read Linux data streams (including FIFO/device nodes).
    // Change time supplements identity/size/mtime but is still not proof of byte equality.
    internal static LogFileObservation Observe(string path)
    {
        var attributes = File.GetAttributes(path);
        if ((attributes & (FileAttributes.Directory | FileAttributes.ReparsePoint | FileAttributes.Device)) != 0)
            throw new IOException("Source must be a regular file without a final symbolic link");
        if (OperatingSystem.IsLinux())
        {
            const uint required = 0x3c1; // type, inode, size, modification and change timestamps.
            if (StatxPath(-100, path, 0x100, 0x7ff, out var info) != 0) throw Failure();
            if ((info.Mask & required) != required) throw new NotSupportedException("Source filesystem lacks required version metadata");
            if ((info.Mode & 0xf000) != 0x8000) throw new IOException("Source is not a regular file");
            var modified = checked(DateTime.UnixEpoch.Ticks + info.ModifiedSeconds * TimeSpan.TicksPerSecond + info.ModifiedNanos / 100);
            if (modified < 0 || modified > DateTime.MaxValue.Ticks || info.ModifiedNanos >= 1000000000 || info.ChangeNanos >= 1000000000)
                throw new IOException("Source timestamps are outside the supported range");
            return new(new(((ulong)info.DeviceMajor << 32) | info.DeviceMinor, info.Inode, 0),
                checked((long)info.Length), modified, info.ChangeSeconds, info.ChangeNanos);
        }
        if (OperatingSystem.IsWindows())
        {
            using var handle = File.OpenHandle(path, FileMode.Open, FileAccess.Read, FileShare.ReadWrite | FileShare.Delete);
            if (GetFileType(handle) != 1) throw new IOException("Source is not a disk file");
            if (!GetFileInformationByHandleEx(handle, 0, out WindowsBasicInfo info, 40)) throw Failure();
            if ((info.Attributes & (uint)(FileAttributes.Directory | FileAttributes.ReparsePoint | FileAttributes.Device)) != 0)
                throw new IOException("Source is not a regular file");
            return new(Read(handle), RandomAccess.GetLength(handle), DateTime.FromFileTimeUtc(info.Modified).Ticks, info.Changed, 0);
        }
        throw new PlatformNotSupportedException("Source observation requires Linux or Windows file identities");
    }

    private static IOException Failure() => new("Cannot read log file identity", new Win32Exception(Marshal.GetLastPInvokeError()));

    // statx has a stable 256-byte kernel ABI; keep the full output buffer even though only identity fields are read.
    [StructLayout(LayoutKind.Explicit, Size = 256)]
    private struct LinuxStatx
    {
        [FieldOffset(0)] internal uint Mask;
        [FieldOffset(28)] internal ushort Mode;
        [FieldOffset(32)] internal ulong Inode;
        [FieldOffset(40)] internal ulong Length;
        [FieldOffset(96)] internal long ChangeSeconds;
        [FieldOffset(104)] internal uint ChangeNanos;
        [FieldOffset(112)] internal long ModifiedSeconds;
        [FieldOffset(120)] internal uint ModifiedNanos;
        [FieldOffset(136)] internal uint DeviceMajor;
        [FieldOffset(140)] internal uint DeviceMinor;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct WindowsFileId
    {
        internal ulong Volume;
        internal ulong Low;
        internal ulong High;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct WindowsBasicInfo
    {
        internal long Created, Accessed, Modified, Changed;
        internal uint Attributes;
    }

    [DllImport("libc", EntryPoint = "statx", SetLastError = true)]
    private static extern int StatxPath(int directory, [MarshalAs(UnmanagedType.LPUTF8Str)] string path,
        int flags, uint mask, out LinuxStatx info);

    [DllImport("kernel32.dll", ExactSpelling = true, SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool GetFileInformationByHandleEx(SafeFileHandle handle, int informationClass,
        out WindowsBasicInfo info, uint size);

    [DllImport("kernel32.dll", ExactSpelling = true, SetLastError = true)]
    private static extern uint GetFileType(SafeFileHandle handle);

    [DllImport("libc", EntryPoint = "statx", SetLastError = true)]
    private static extern int Statx(SafeFileHandle handle, [MarshalAs(UnmanagedType.LPUTF8Str)] string path,
        int flags, uint mask, out LinuxStatx info);

    [DllImport("kernel32.dll", ExactSpelling = true, SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool GetFileInformationByHandleEx(SafeFileHandle handle, int informationClass,
        out WindowsFileId info, uint size);
}

// A point-in-time observation. S2 must bind/verify the bytes actually read, not trust this stamp alone.
internal readonly record struct LogFileObservation(LogFileIdentity Identity, long Length, long ModifiedTicks, long ChangeStamp, uint ChangeFraction);
