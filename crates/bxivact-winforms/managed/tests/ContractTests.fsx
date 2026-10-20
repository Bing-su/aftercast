#r "../System.Windows.Forms.dll"

open System
open System.Windows.Forms

// Check the public managed contract without loading ACT, its bridge, or any game plugin.
let check condition = if not condition then failwith "WinForms compatibility contract check failed"
let assembly = typeof<TabPage>.Assembly
let identity = assembly.GetName()
check (identity.Name = "System.Windows.Forms")
check (identity.Version = Version(4, 0, 0, 0))
check (identity.GetPublicKeyToken().Length = 0)
check (typeof<Label>.Assembly = assembly)
check (typeof<UserControl>.BaseType = typeof<ContainerControl>)
check (assembly.GetReferencedAssemblies() |> Array.forall (fun reference -> reference.Name <> "Advanced Combat Tracker"))

for create in [ (fun () -> new TabPage() :> Control); (fun () -> new Label() :> Control); (fun () -> new UserControl() :> Control) ] do
    use control = create ()
    let mutable disposedEvents = 0
    control.Disposed.Add(fun _ -> disposedEvents <- disposedEvents + 1)
    check (not control.IsDisposed)
    control.Text <- "Ready"
    check (control.Text = "Ready")
    control.Text <- null
    check (control.Text = "")
    try
        control.Handle |> ignore
        failwith "Unsupported native handle unexpectedly succeeded"
    with :? PlatformNotSupportedException as error ->
        check (error.Message.Contains("native handles"))
    control.Dispose()
    control.Dispose()
    check control.IsDisposed
    check (disposedEvents = 1)

printfn "Independent WinForms identity, status text, disposal and native-handle rejection checks passed."
