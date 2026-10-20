#r "../../crates/bxivact/managed/Advanced Combat Tracker.dll"
#r "../../crates/bxivact/managed/Bxivact.Bridge.dll"

open System
open System.Runtime.InteropServices
open Bxivact.Bridge

let check condition = if not condition then failwith "Managed bridge contract check failed"

// Use real allocated memory to verify the same offsets the Rust repr(C) packet uses.
let buffer = Marshal.AllocHGlobal(24)
try
    check (EntryPoint.Invoke(IntPtr.Zero, 24) = 1)
    check (EntryPoint.Invoke(buffer, 23) = 1)
    Marshal.WriteInt32(buffer, 0, 24)
    Marshal.WriteInt32(buffer, 4, 2)
    check (EntryPoint.Invoke(buffer, 24) = 2)
    Marshal.WriteInt32(buffer, 4, 1)
    Marshal.WriteInt32(buffer, 8, 99)
    Marshal.WriteInt64(buffer, 16, 0L)
    check (EntryPoint.Invoke(buffer, 24) = 6)
    check (EntryPoint.ReadError(IntPtr.Zero, 0) = -1)
    let expectedError = "Unknown bridge command: 99"
    let required = EntryPoint.ReadError(buffer, 1)
    check (required = System.Text.Encoding.UTF8.GetByteCount(expectedError))
    check (Marshal.ReadInt32(buffer, 0) = 24)
    let errorBuffer = Marshal.AllocHGlobal(required)
    try
        check (EntryPoint.ReadError(errorBuffer, required) = required)
        check (Marshal.PtrToStringUTF8(errorBuffer, required) = expectedError)
    finally
        Marshal.FreeHGlobal(errorBuffer)
    Marshal.WriteInt32(buffer, 8, 2)
    check (EntryPoint.Invoke(buffer, 24) = 4)
    Marshal.WriteInt32(buffer, 8, 1)
    check (EntryPoint.Invoke(buffer, 24) = 0)
    check (EntryPoint.ReadError(buffer, 0) = 0)
    let session = Marshal.ReadInt64(buffer, 16)
    check (session > 0L)
    check (Marshal.ReadInt32(buffer, 12) = 10)
    check (EntryPoint.Invoke(buffer, 24) = 3)
    Marshal.WriteInt32(buffer, 8, 3)
    Marshal.WriteInt64(buffer, 16, session + 1L)
    check (EntryPoint.Invoke(buffer, 24) = 5)
    Marshal.WriteInt64(buffer, 16, session)
    check (EntryPoint.Invoke(buffer, 24) = 0)
    check (EntryPoint.Invoke(buffer, 24) = 4)
    printfn "Bridge ABI validation and session ownership checks passed."
finally
    Marshal.FreeHGlobal(buffer)
