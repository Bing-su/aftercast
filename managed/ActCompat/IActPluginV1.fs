namespace Advanced_Combat_Tracker

open System.Windows.Forms

/// Preserve the original ACT interface, including its WinForms parameter types.
type IActPluginV1 =
    abstract InitPlugin: pluginScreenSpace: TabPage * pluginStatusText: Label -> unit
    abstract DeInitPlugin: unit -> unit
