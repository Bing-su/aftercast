namespace Bxivact.Bridge

open System
open System.IO
open System.Reflection
open System.Runtime.Loader

module internal Platform =
    let prepare () =
        let bridge = Assembly.GetExecutingAssembly()
        let directory = Path.GetDirectoryName(bridge.Location)
        // Costura byte-loaded dependencies bind through Default, outside the plugin/bridge contexts.
        for name in [ "System.Windows.Extensions.dll"; "System.Security.Permissions.dll" ] do
            AssemblyLoadContext.Default.LoadFromAssemblyPath(Path.Combine(directory, name)) |> ignore
        if OperatingSystem.IsLinux() then
            let context = AssemblyLoadContext.GetLoadContext(bridge)
            let path = Path.Combine(directory, "System.Windows.Forms.dll")
            // Bind the unsigned headless model in the bridge's context without opening native windows.
            context.LoadFromAssemblyPath(path) |> ignore
