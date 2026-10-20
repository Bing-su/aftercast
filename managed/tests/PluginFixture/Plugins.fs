namespace Bxivact.PluginFixture

open System
open Advanced_Combat_Tracker

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
    member _.Control = control
    interface IActPluginV1 with
        member _.InitPlugin(_, _) = ()
        member _.DeInitPlugin() = ()
