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
            if (!GetFileInformationByHandleEx(handle, 18, out var info, 24)) throw Failure();
            if (info.Low == 0 && info.High == 0) throw new NotSupportedException("Log filesystem does not supply file identities");
            return new(info.Volume, info.Low, info.High);
        }
        throw new PlatformNotSupportedException("Log following requires Linux or Windows file identities");
    }

    private static IOException Failure() => new("Cannot read log file identity", new Win32Exception(Marshal.GetLastPInvokeError()));

    // statx has a stable 256-byte kernel ABI; keep the full output buffer even though only identity fields are read.
    [StructLayout(LayoutKind.Explicit, Size = 256)]
    private struct LinuxStatx
    {
        [FieldOffset(0)] internal uint Mask;
        [FieldOffset(32)] internal ulong Inode;
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

    [DllImport("libc", EntryPoint = "statx", SetLastError = true)]
    private static extern int Statx(SafeFileHandle handle, [MarshalAs(UnmanagedType.LPUTF8Str)] string path,
        int flags, uint mask, out LinuxStatx info);

    [DllImport("kernel32.dll", ExactSpelling = true, SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool GetFileInformationByHandleEx(SafeFileHandle handle, int informationClass,
        out WindowsFileId info, uint size);
}
