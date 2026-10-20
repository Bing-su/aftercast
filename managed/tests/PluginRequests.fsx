#r "../../target/managed/test-bridge/Advanced Combat Tracker.dll"
#r "../../target/managed/test-bridge/Bxivact.Bridge.dll"

open System
open System.IO
open System.Runtime.InteropServices
open System.Text
open Bxivact.Bridge

let check condition = if not condition then failwith "Plugin request boundary check failed"
let memory = Marshal.AllocHGlobal(65537)
let diagnostic () =
    let output = Marshal.AllocHGlobal(8192)
    try
        let length = EntryPoint.ReadError(output, 8192)
        check (length >= 0 && length <= 8192)
        Marshal.PtrToStringUTF8(output, length)
    finally Marshal.FreeHGlobal(output)
try
    Marshal.WriteInt32(memory, 0, 24)
    Marshal.WriteInt32(memory, 4, 1)
    Marshal.WriteInt32(memory, 8, 1)
    check (EntryPoint.Invoke(memory, 24) = 0)
    let session = Marshal.ReadInt64(memory, 16)
    let path = Path.GetFullPath("managed/tests/PluginFixture/bin/Release/net10.0/Bxivact.PluginFixture.dll")
    let pathBytes = Encoding.UTF8.GetBytes(path)
    let typeBytes = Encoding.UTF8.GetBytes("Bxivact.PluginFixture.GoodPlugin")
    let size = 40 + pathBytes.Length + typeBytes.Length
    let reset () =
        Marshal.WriteInt32(memory, 0, size)
        Marshal.WriteInt32(memory, 4, 1)
        Marshal.WriteInt64(memory, 8, session)
        Marshal.WriteInt32(memory, 16, 40)
        Marshal.WriteInt32(memory, 20, pathBytes.Length)
        Marshal.WriteInt32(memory, 24, 40 + pathBytes.Length)
        Marshal.WriteInt32(memory, 28, typeBytes.Length)
        Marshal.WriteInt64(memory, 32, 0L)
        Marshal.Copy(pathBytes, 0, IntPtr.Add(memory, 40), pathBytes.Length)
        Marshal.Copy(typeBytes, 0, IntPtr.Add(memory, 40 + pathBytes.Length), typeBytes.Length)
    let rejected change expected =
        reset ()
        change ()
        check (EntryPoint.CreatePlugin(memory, size) = -1)
        check ((diagnostic ()).Contains(expected: string))
        check (Marshal.ReadInt64(memory, 32) = 0L)
    check (EntryPoint.CreatePlugin(IntPtr.Zero, 40) = -1)
    check (EntryPoint.CreatePlugin(memory, 39) = -1)
    check (EntryPoint.CreatePlugin(memory, 65537) = -1)
    rejected (fun () -> Marshal.WriteInt32(memory, 4, 2)) "size/version"
    rejected (fun () -> Marshal.WriteInt64(memory, 8, session + 1L)) "does not own"
    rejected (fun () -> Marshal.WriteInt32(memory, 16, Int32.MaxValue)) "outside"
    rejected (fun () -> Marshal.WriteInt32(memory, 20, Int32.MaxValue)) "outside"
    rejected (fun () -> Marshal.WriteInt32(memory, 28, -1)) "outside"
    rejected (fun () -> Marshal.WriteInt32(memory, 24, 0)) "outside"
    rejected (fun () -> Marshal.WriteByte(memory, 40, 0uy)) "NUL"
    rejected (fun () -> Marshal.WriteByte(memory, 40, 255uy)) "translate"
    reset ()
    check (EntryPoint.CreatePlugin(memory, size) = 0)
    check (Marshal.ReadInt64(memory, 32) > 0L)
    check (diagnostic () = "")
    let instance = Marshal.ReadInt64(memory, 32)
    let command () =
        Marshal.WriteInt32(memory, 0, 8224)
        Marshal.WriteInt32(memory, 4, 1)
        Marshal.WriteInt64(memory, 8, session)
        Marshal.WriteInt64(memory, 16, instance)
        Marshal.WriteInt32(memory, 24, 99)
        Marshal.WriteInt32(memory, 28, 0)
    check (EntryPoint.PluginCommand(IntPtr.Zero, 8224) = -1)
    check (EntryPoint.PluginCommand(memory, 8223) = -1)
    check (EntryPoint.PluginCommand(memory, 8225) = -1)
    command ()
    Marshal.WriteInt32(memory, 4, 2)
    check (EntryPoint.PluginCommand(memory, 8224) = -1)
    check ((diagnostic ()).Contains("size/version"))
    command ()
    Marshal.WriteInt64(memory, 8, session + 1L)
    check (EntryPoint.PluginCommand(memory, 8224) = -1)
    check ((diagnostic ()).Contains("does not own"))
    command ()
    check (EntryPoint.PluginCommand(memory, 8224) = -1)
    check ((diagnostic ()).Contains("Unknown plugin lifecycle command"))
    command ()
    Marshal.WriteInt32(memory, 24, 1)
    Marshal.WriteInt64(memory, 16, Int64.MaxValue)
    check (EntryPoint.PluginCommand(memory, 8224) = -1)
    check ((diagnostic ()).Contains("Unknown plugin instance"))
    Marshal.WriteInt32(memory, 0, 24)
    Marshal.WriteInt32(memory, 4, 1)
    Marshal.WriteInt32(memory, 8, 3)
    Marshal.WriteInt64(memory, 16, session)
    check (EntryPoint.Invoke(memory, 24) = 0)
    reset ()
    check (EntryPoint.CreatePlugin(memory, size) = -1)
    check ((diagnostic ()).Contains("No managed ACT session"))
    printfn "Plugin ABI bounds, strict UTF-8, ownership, and failure recovery passed."
finally Marshal.FreeHGlobal(memory)
