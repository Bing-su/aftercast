namespace Bxivact.Bridge

open System
open System.Runtime.InteropServices
open Bxivact.ActCompat

/// Uses the standard ComponentEntryPoint signature so F# needs no UnmanagedCallersOnly support.
[<AbstractClass; Sealed>]
type EntryPoint private () =
    static let gate = obj ()
    static let mutable active: HostSession option = None

    static member Invoke(buffer: nativeint, sizeBytes: int) : int =
        // Validate the fixed header before reading fields; Rust owns it for this call only.
        try
            if buffer = IntPtr.Zero || sizeBytes <> 24 then 1
            elif Marshal.ReadInt32(buffer, 0) <> 24 || Marshal.ReadInt32(buffer, 4) <> 1 then 2
            else
                lock gate (fun () ->
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
                    | _ -> 6)
        // Managed exceptions must become status codes rather than escape into native callers.
        with _ -> -1
