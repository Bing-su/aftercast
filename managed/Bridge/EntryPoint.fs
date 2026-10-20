namespace Bxivact.Bridge

open System
open System.Runtime.InteropServices
open System.Text
open Bxivact.ActCompat

/// Uses ComponentEntryPoint delegates so F# needs no UnmanagedCallersOnly support.
[<AbstractClass; Sealed>]
type EntryPoint private () =
    static let gate = obj ()
    static let mutable active: HostSession option = None
    static let mutable lastError = String.Empty

    static member private Describe(error: exn) =
        // Exception.Message can itself be overridden; diagnostic formatting must not escape either.
        try
            let text = error.GetType().FullName + ": " + error.Message
            if text.Length > 2048 then text.Substring(0, 2048) else text
        with _ -> "Managed exception (diagnostic formatting failed)"

    static member private Dispatch(buffer: nativeint) =
        let command = Marshal.ReadInt32(buffer, 8)
        let sessionId = uint64 (Marshal.ReadInt64(buffer, 16))
        Marshal.WriteInt32(buffer, 12, Environment.Version.Major)
        match command, active with
        | 1, None ->
            let session = HostSession()
            active <- Some session
            Marshal.WriteInt64(buffer, 16, int64 session.Id)
            0
        | 1, Some _ -> 3
        | (2 | 3), None -> 4
        | (2 | 3), Some session when session.Id <> sessionId -> 5
        | 2, Some _ -> 0
        | 3, Some _ ->
            active <- None
            0
        | _ -> 6

    static member Invoke(buffer: nativeint, sizeBytes: int) : int =
        try
            lock gate (fun () ->
                lastError <- String.Empty
                // Check the fixed header before reading fields; Rust owns it for this call only.
                let status =
                    if buffer = IntPtr.Zero || sizeBytes <> 24 then 1
                    elif Marshal.ReadInt32(buffer, 0) <> 24 || Marshal.ReadInt32(buffer, 4) <> 1 then 2
                    else EntryPoint.Dispatch(buffer)
                if status <> 0 then
                    lastError <-
                        match status with
                        | 1 -> "Invalid lifecycle buffer: expected a non-null 24-byte header"
                        | 2 -> "Lifecycle ABI mismatch: expected header size 24 and version 1"
                        | 3 -> "A managed ACT session is already active"
                        | 4 -> "No managed ACT session is active"
                        | 5 -> "The request does not own the active managed ACT session"
                        | _ -> sprintf "Unknown bridge command: %d" (Marshal.ReadInt32(buffer, 8))
                status)
        // Cover lock acquisition as well as command handling at the native boundary.
        with error ->
            lastError <- EntryPoint.Describe(error)
            -1

    /// Returns required UTF-8 length; insufficient capacity writes nothing and preserves the error.
    static member ReadError(buffer: nativeint, capacity: int) : int =
        try
            lock gate (fun () ->
                if buffer = IntPtr.Zero || capacity < 0 then -1
                else
                    let bytes = Encoding.UTF8.GetBytes(lastError)
                    if capacity >= bytes.Length then Marshal.Copy(bytes, 0, buffer, bytes.Length)
                    bytes.Length)
        with _ -> -1
