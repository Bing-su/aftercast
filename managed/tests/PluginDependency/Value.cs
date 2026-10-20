using System.Reflection;
using System.Runtime.CompilerServices;

namespace Bxivact.PluginDependency;

public static class Value
{
    // Keep the cross-assembly constructor dependency observable under optimized JIT compilation.
    public static int Number { [MethodImpl(MethodImplOptions.NoInlining)] get => 42; }
    // Exercise legacy binding from both plugin and Costura-like byte-loaded contexts.
    public static void CheckPermissions()
    {
        var assembly = Assembly.Load("System.Security.Permissions, Version=0.0.0.0, Culture=neutral, PublicKeyToken=cc7b13ffcd2ddd51");
        var permission = assembly.GetType("System.Security.Permissions.SecurityPermission", true)!;
        var constructor = permission.GetConstructors()[0];
        var state = Enum.ToObject(constructor.GetParameters()[0].ParameterType, 0);
        var instance = constructor.Invoke([state]);
        permission.GetMethod("Demand")!.Invoke(instance, []);
    }
}
