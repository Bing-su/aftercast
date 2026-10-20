namespace Bxivact.LegacyReference

open Advanced_Combat_Tracker

/// Emit real ldsfld/stsfld instructions against the original ACT, not reflection or our F# declarations.
type HostProbe private () =
    static member Root() : obj = box ActGlobals.oFormActMain
    static member Character(value: string) =
        ActGlobals.charName <- value
        ActGlobals.charName
    static member AppDataPath() = ActGlobals.oFormActMain.AppDataFolder.FullName
    static member MissingRegistration() = isNull (ActGlobals.oFormActMain.PluginGetSelfData(null))
    static member Restart() = ActGlobals.oFormActMain.RestartACT(false, "fixture restart")
    static member BackgroundDiagnostic() =
        let thread = System.Threading.Thread(System.Threading.ThreadStart(fun () ->
            ActGlobals.oFormActMain.WriteExceptionLog(System.InvalidOperationException("background diagnostic"), "probe")))
        thread.Start()
        thread.Join()
