namespace Bxivact.PluginFixture

open System
open Advanced_Combat_Tracker
open System.IO
open System.Windows.Forms

/// Loading and construction must not require creating controls or calling InitPlugin.
type GoodPlugin() =
    let value = Bxivact.PluginDependency.Value.Number
    do if value <> 42 then failwith "Dependency returned the wrong value"
    member _.Value = value
    interface IActPluginV1 with
        member _.InitPlugin(_, _) = failwith "InitPlugin must not run during construction"
        member _.DeInitPlugin() = failwith "Uninitialized plugins must not receive DeInitPlugin"

type ThrowingPlugin() =
    do raise (InvalidOperationException("fixture constructor failure"))
    interface IActPluginV1 with
        member _.InitPlugin(_, _) = ()
        member _.DeInitPlugin() = ()

type NeedsArgument(value: int) =
    member _.Value = value
    interface IActPluginV1 with
        member _.InitPlugin(_, _) = ()
        member _.DeInitPlugin() = ()

type GenericPlugin<'T>() =
    interface IActPluginV1 with
        member _.InitPlugin(_, _) = ()
        member _.DeInitPlugin() = ()

type UiPlugin() =
    let control = new System.Windows.Forms.TabPage()
    do control.Handle |> ignore
    member _.Control = control
    interface IActPluginV1 with
        member _.InitPlugin(_, _) = ()
        member _.DeInitPlugin() = ()

/// Persist lifecycle evidence so Rust observes callbacks, field binding and disposal rather than a mock.
type LifecyclePlugin(mode: string) =
    static let mutable failedInstance: IActPluginV1 option = None
    let mutable folder = ""
    let mutable controls: (TabPage * Label) option = None
    interface IActPluginV1 with
        member this.InitPlugin(tab, status) =
            // Compile against the genuine WinForms reference and execute against the Linux adapter.
            use settings = new UserControl()
            settings.Text <- "Settings"
            if settings.IsDisposed || settings.Text <> "Settings" then failwith "UserControl contract failed"
            Bxivact.PluginDependency.Value.CheckPermissions()
            let dependency = Path.Combine(Path.GetDirectoryName(this.GetType().Assembly.Location), "Bxivact.PluginDependency.dll")
            let embedded = System.Reflection.Assembly.Load(File.ReadAllBytes(dependency))
            embedded.GetType("Bxivact.PluginDependency.Value").GetMethod("CheckPermissions").Invoke(null, [||]) |> ignore
            let globals = typeof<IActPluginV1>.Assembly.GetType("Advanced_Combat_Tracker.ActGlobals", true)
            let root = globals.GetField("oFormActMain").GetValue(null)
            failedInstance |> Option.iter (fun failed ->
                if not (isNull (root.GetType().GetMethod("PluginGetSelfData").Invoke(root, [| failed |]))) then
                    failwith "Failed plugin registration leaked")
            failedInstance <- None
            let data = root.GetType().GetMethod("PluginGetSelfData").Invoke(root, [| this |])
            if isNull data then failwith "Plugin registration is missing during InitPlugin"
            let own = data.GetType()
            if not (Object.ReferenceEquals(own.GetField("tpPluginSpace").GetValue(data), tab)) then failwith "Wrong plugin tab"
            if not (Object.ReferenceEquals(own.GetField("lblPluginStatus").GetValue(data), status)) then failwith "Wrong status label"
            let pluginFile = own.GetField("pluginFile").GetValue(data) :?> FileInfo
            if pluginFile.FullName <> this.GetType().Assembly.Location then failwith "Wrong plugin file"
            folder <- (root.GetType().GetProperty("AppDataFolder").GetValue(root) :?> DirectoryInfo).FullName
            controls <- Some (tab, status)
            let disposed = EventHandler(fun _ _ ->
                File.AppendAllText(Path.Combine(folder, mode + ".disposed"), "disposed\n")
                if mode = "deinit" then failwith "fixture disposal failure")
            tab.Disposed.AddHandler(disposed)
            status.Disposed.AddHandler(disposed)
            File.AppendAllText(Path.Combine(folder, mode + ".init"), "init\n")
            status.Text <- "Ready"
            if mode = "background" then
                let thread = Threading.Thread(Threading.ThreadStart(fun () ->
                    root.GetType().GetMethod("WriteExceptionLog").Invoke(root, [| InvalidOperationException("unrelated background diagnostic"); "background" |]) |> ignore))
                thread.Start()
                thread.Join()
            if mode = "long" then status.Text <- String.replicate 5000 "x"
            if mode = "throw" then
                failedInstance <- Some (this :> IActPluginV1)
                failwith "fixture initialization failure"
            if mode = "logged" then
                failedInstance <- Some (this :> IActPluginV1)
                root.GetType().GetMethod("WriteExceptionLog").Invoke(root, [| InvalidOperationException("fixture caught failure"); "fixture" |]) |> ignore
        member _.DeInitPlugin() =
            match controls with
            | Some (tab, status) when not tab.IsDisposed && not status.IsDisposed -> ()
            | _ -> failwith "Controls were disposed before DeInitPlugin"
            File.AppendAllText(Path.Combine(folder, mode + ".deinit"), "deinit\n")
            if mode = "deinit" then failwith "fixture shutdown failure"

type ReadyPlugin() = inherit LifecyclePlugin("ready")
type InitFailurePlugin() = inherit LifecyclePlugin("throw")
type LoggedFailurePlugin() = inherit LifecyclePlugin("logged")
type ShutdownFailurePlugin() = inherit LifecyclePlugin("deinit")
type LongStatusPlugin() = inherit LifecyclePlugin("long")
type BackgroundLogPlugin() = inherit LifecyclePlugin("background")
