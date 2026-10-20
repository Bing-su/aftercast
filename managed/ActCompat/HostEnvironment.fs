namespace Advanced_Combat_Tracker

open System
open System.Collections.Generic
open System.IO
open System.Windows.Forms

/// Preserve the original public fields: properties would not satisfy plugin MemberRefs.
[<AllowNullLiteral>]
type ActPluginData() =
    [<DefaultValue>] val mutable public pluginFile: FileInfo
    [<DefaultValue>] val mutable public pluginObj: IActPluginV1
    [<DefaultValue>] val mutable public tpPluginSpace: TabPage
    [<DefaultValue>] val mutable public lblPluginStatus: Label

/// A bounded startup contract, not ACT's combat engine or settings window.
[<AllowNullLiteral>]
type FormActMain(appDataFolder: DirectoryInfo) =
    inherit Form()
    let plugins = Dictionary<obj, ActPluginData>(ReferenceEqualityComparer.Instance)
    let errorGate = obj ()
    let ownerThread = Environment.CurrentManagedThreadId
    let mutable errorCount = 0L
    let mutable lastError = ""
    let mutable ownerErrorCount = 0L
    let mutable lastOwnerError = ""
    member _.AppDataFolder = appDataFolder
    member _.PluginGetSelfData(plugin: IActPluginV1) =
        if Object.ReferenceEquals(plugin, null) then null
        else match plugins.TryGetValue(plugin) with true, data -> data | _ -> null
    member _.WriteExceptionLog(error: Exception, context: string) =
        // Preserve a bounded diagnostic even when InitPlugin catches its own exception and returns.
        lock errorGate (fun () ->
            errorCount <- errorCount + 1L
            let message = sprintf "%s: %s" context (if isNull error then "Unknown exception" else error.Message)
            lastError <- if message.Length > 2048 then message.Substring(0, 2048) else message
            if Environment.CurrentManagedThreadId = ownerThread then
                ownerErrorCount <- ownerErrorCount + 1L
                lastOwnerError <- lastError)
    member _.RestartACT(_: bool, _: string) : unit =
        raise (PlatformNotSupportedException("RestartACT is not supported by an embedded host"))
    member internal _.Register(data: ActPluginData) = plugins.Add(data.pluginObj, data)
    member internal _.Unregister(plugin: IActPluginV1) = plugins.Remove(plugin) |> ignore
    member internal _.Errors = lock errorGate (fun () -> errorCount, lastError)
    member internal _.OwnerErrors = lock errorGate (fun () -> ownerErrorCount, lastOwnerError)

[<AbstractClass; Sealed>]
type ActGlobals private () =
    // F# requires private static val fields; the maintainer publish step exposes these two CLI fields.
    [<DefaultValue>] static val mutable private oFormActMain: FormActMain
    [<DefaultValue>] static val mutable private charName: string
    static member internal Host with get () = ActGlobals.oFormActMain and set value = ActGlobals.oFormActMain <- value
    static member internal Character with set value = ActGlobals.charName <- value

namespace Bxivact.ActCompat

open System
open System.IO
open Advanced_Combat_Tracker

/// Own the startup registry and globals separately from the original public ACT contract.
type HostEnvironment(directory: string) =
    let main = new FormActMain(DirectoryInfo(directory))
    do
        if not (isNull ActGlobals.Host) then
            main.Dispose()
            invalidOp "ACT globals are already owned by another environment"
        ActGlobals.Host <- main
        ActGlobals.Character <- ""
    member _.Register(data: ActPluginData) = main.Register(data)
    member _.Unregister(plugin: IActPluginV1) = main.Unregister(plugin)
    member _.Errors = main.OwnerErrors
    member _.Diagnostics = main.Errors
    interface IDisposable with
        member _.Dispose() =
            // Clear globals before disposal so no later session can observe a disposed root.
            ActGlobals.Host <- null
            ActGlobals.Character <- null
            main.Dispose()
