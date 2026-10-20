namespace Bxivact.Bridge

open System
open System.Collections.Generic
open System.IO
open System.Threading
open System.Windows.Forms
open Advanced_Combat_Tracker
open Bxivact.ActCompat

type private PluginState = Constructed | Initializing | Initialized | Failed | Stopped
type private PluginRecord =
    { Plugin: IActPluginV1; Path: string
      mutable State: PluginState; mutable Controls: (TabPage * Label) option }

/// Serialize lifecycle calls through EntryPoint's gate and never retry a failed callback implicitly.
type internal PluginSession() =
    let plugins = Dictionary<int64, PluginRecord>()
    let initialized = ResizeArray<int64>()
    let mutable nextId = 0L
    let mutable directory: string option = None
    let mutable environment: HostEnvironment option = None
    let find id =
        match plugins.TryGetValue(id) with true, plugin -> plugin | _ -> invalidArg "id" "Unknown plugin instance"
    let host () =
        match environment with
        | Some host -> host
        | None ->
            if OperatingSystem.IsWindows() && Thread.CurrentThread.GetApartmentState() <> ApartmentState.STA then
                invalidOp "Windows plugin initialization requires a caller-owned STA thread and UI message loop"
            let path = directory |> Option.defaultWith (fun () -> invalidOp "Host data directory has not been configured")
            let host = new HostEnvironment(path)
            environment <- Some host
            host
    let cleanup plugin =
        match plugin.Controls with
        | None -> ()
        | Some (tab, label) ->
            plugin.Controls <- None
            environment |> Option.iter (fun host -> host.Unregister(plugin.Plugin))
            let errors = ResizeArray<exn>()
            for dispose in [ (fun () -> tab.Dispose()); (fun () -> label.Dispose()) ] do
                try dispose () with error -> errors.Add(error)
            if errors.Count <> 0 then raise (AggregateException("Host control disposal failed", errors))
    member _.Configure(path: string) =
        if directory.IsSome then invalidOp "Host environment is already configured"
        if not (Path.IsPathFullyQualified(path)) || not (Directory.Exists(path)) then
            invalidArg "path" "Host data directory must be an existing absolute directory"
        directory <- Some path
    member _.Add(path, instance: obj) =
        nextId <- nextId + 1L
        plugins.Add(nextId, { Plugin = instance :?> IActPluginV1; Path = path; State = Constructed; Controls = None })
        nextId
    member _.Initialize(id) =
        let plugin = find id
        if plugin.State <> Constructed then invalidOp "Plugin initialization can only be attempted once per instance"
        plugin.State <- Initializing
        try
            let host = host ()
            let data = ActPluginData()
            data.pluginObj <- plugin.Plugin
            data.pluginFile <- FileInfo(plugin.Path)
            data.tpPluginSpace <- new TabPage()
            try data.lblPluginStatus <- new Label() with _ -> data.tpPluginSpace.Dispose(); reraise ()
            let tab, label = data.tpPluginSpace, data.lblPluginStatus
            plugin.Controls <- Some (tab, label)
            host.Register(data)
            let errors, _ = host.Errors
            plugin.Plugin.InitPlugin(tab, label)
            let after, message = host.Errors
            if after <> errors then invalidOp ("InitPlugin reported an exception: " + message)
            plugin.State <- Initialized
            initialized.Add(id)
            label.Text
        with error ->
            plugin.State <- Failed
            let failure = try cleanup plugin; error with cleanupError -> AggregateException(error, cleanupError) :> exn
            raise (InvalidOperationException(sprintf "InitPlugin failed for %s: %s" plugin.Path (failure.GetBaseException().Message), failure))
    member _.Deinitialize(id) =
        let plugin = find id
        if plugin.State <> Initialized then invalidOp "Only an initialized plugin can be deinitialized"
        plugin.State <- Stopped
        let errors = ResizeArray<exn>()
        try plugin.Plugin.DeInitPlugin() with error -> errors.Add(error)
        try cleanup plugin with error -> errors.Add(error)
        if errors.Count <> 0 then raise (AggregateException("DeInitPlugin failed for " + plugin.Path, errors))
    member this.Stop() =
        let errors = ResizeArray<exn>()
        for id in Seq.rev initialized do
            if (find id).State = Initialized then
                try this.Deinitialize(id) with error -> errors.Add(error)
        plugins.Clear()
        initialized.Clear()
        directory <- None
        let previous = environment
        environment <- None
        previous |> Option.iter (fun host -> try (host :> IDisposable).Dispose() with error -> errors.Add(error))
        if errors.Count <> 0 then raise (AggregateException("Plugin shutdown failed; the session was closed", errors))
