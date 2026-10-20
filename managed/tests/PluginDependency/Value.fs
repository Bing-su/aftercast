namespace Bxivact.PluginDependency

/// Force the constructor to resolve a deployment dependency rather than a bridge dependency.
type Value private () =
    static member Number = 42
    // Exercise the legacy binding from both a plugin context and a Costura-like byte-loaded assembly.
    // Keep this method BCL-only: the byte-loaded context deliberately has no FSharp.Core resolver.
    static member CheckPermissions() =
        let assembly = System.Reflection.Assembly.Load("System.Security.Permissions, Version=0.0.0.0, Culture=neutral, PublicKeyToken=cc7b13ffcd2ddd51")
        let permission = assembly.GetType("System.Security.Permissions.SecurityPermission", true)
        let constructor = permission.GetConstructors().[0]
        let state = System.Enum.ToObject(constructor.GetParameters().[0].ParameterType, 0)
        let instance = constructor.Invoke([| state |])
        let _ = permission.GetMethod("Demand").Invoke(instance, [||])
        ()
