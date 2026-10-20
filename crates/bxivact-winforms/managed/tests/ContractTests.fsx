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
check (assembly.GetReferencedAssemblies() |> Array.forall (fun reference -> reference.Name <> "Advanced Combat Tracker"))

for create in [ (fun () -> new TabPage() :> obj); (fun () -> new Label() :> obj) ] do
    try
        create () |> ignore
        failwith "Unsupported UI construction unexpectedly succeeded"
    with :? PlatformNotSupportedException as error ->
        check (error.Message.Contains("Linux WinForms UI is not implemented"))

printfn "Independent WinForms signature identity and explicit UI-failure checks passed."
