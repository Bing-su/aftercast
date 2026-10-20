#r "../../crates/bxivact/managed/Bxivact.ActCompat.dll"
#r "../../crates/bxivact/managed/Bxivact.Bridge.dll"

open System
open System.Runtime.InteropServices
open Bxivact.Bridge

// Use real allocated memory to verify the same offsets the Rust repr(C) packet uses.
let buffer = Marshal.AllocHGlobal(24)
try
    assert (EntryPoint.Invoke(IntPtr.Zero, 24) = 1)
    assert (EntryPoint.Invoke(buffer, 23) = 1)
    Marshal.WriteInt32(buffer, 0, 24)
    Marshal.WriteInt32(buffer, 4, 2)
    assert (EntryPoint.Invoke(buffer, 24) = 2)
    Marshal.WriteInt32(buffer, 4, 1)
    Marshal.WriteInt32(buffer, 8, 99)
    Marshal.WriteInt64(buffer, 16, 0L)
    assert (EntryPoint.Invoke(buffer, 24) = 6)
    Marshal.WriteInt32(buffer, 8, 2)
    assert (EntryPoint.Invoke(buffer, 24) = 4)
    Marshal.WriteInt32(buffer, 8, 1)
    assert (EntryPoint.Invoke(buffer, 24) = 0)
    let session = Marshal.ReadInt64(buffer, 16)
    assert (session > 0L)
    assert (Marshal.ReadInt32(buffer, 12) = 10)
    assert (EntryPoint.Invoke(buffer, 24) = 3)
    Marshal.WriteInt32(buffer, 8, 3)
    Marshal.WriteInt64(buffer, 16, session + 1L)
    assert (EntryPoint.Invoke(buffer, 24) = 5)
    Marshal.WriteInt64(buffer, 16, session)
    assert (EntryPoint.Invoke(buffer, 24) = 0)
    assert (EntryPoint.Invoke(buffer, 24) = 4)
    printfn "Bridge ABI validation and session ownership checks passed."
finally
    Marshal.FreeHGlobal(buffer)
