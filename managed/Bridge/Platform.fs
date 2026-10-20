namespace Bxivact.Bridge

open System
open System.IO
open System.Reflection
open System.Runtime.Loader

module internal Platform =
    let prepare () =
        if OperatingSystem.IsLinux() then
            let bridge = Assembly.GetExecutingAssembly()
            let context = AssemblyLoadContext.GetLoadContext(bridge)
            let path = Path.Combine(Path.GetDirectoryName(bridge.Location), "System.Windows.Forms.dll")
            // This unsigned adapter supplies signature types only, not WinForms controls or UI behavior.
            context.LoadFromAssemblyPath(path) |> ignore
