namespace Bxivact.PluginDependency

/// Force the constructor to resolve a deployment dependency rather than a bridge dependency.
type Value private () =
    static member Number = 42
