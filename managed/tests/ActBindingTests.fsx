#r "../../crates/bxivact-winforms/managed/System.Windows.Forms.dll"
#r "../../crates/bxivact/managed/Advanced Combat Tracker.dll"
#r "LegacyReference/bin/Release/net10.0/Bxivact.LegacyReference.dll"

open System
open System.IO
open Advanced_Combat_Tracker
open Bxivact.LegacyReference

// These checks must execute in Release too; Debug.Assert-style checks are insufficient here.
let check condition = if not condition then failwith "ACT binary-reference binding check failed"
let expectedIdentity = "Advanced Combat Tracker, Version=3.8.5.288, Culture=neutral, PublicKeyToken=a946b61e93d97868"
let reference = typeof<ContractProbe>.Assembly.GetReferencedAssemblies()
                |> Array.find (fun item -> item.Name = "Advanced Combat Tracker")
check (reference.FullName = expectedIdentity)
let actual = typeof<Dnum>.Assembly
check (actual.FullName = expectedIdentity)
let expectedPath = Path.GetFullPath(Path.Combine(__SOURCE_DIRECTORY__, "../../crates/bxivact/managed/Advanced Combat Tracker.dll"))
check (actual.Location = expectedPath)
check (not typeof<Dnum>.IsValueType && not typeof<Dnum>.IsSealed)
check (ContractProbe.Run() = 123L)
check (isNull (typeof<Dnum>.GetProperty("Number").GetSetMethod()))
check (Dnum(-11L).Number = -11L)

// A missing semantic implementation must be visible rather than falling back to Object behavior.
try
    Dnum(1L).ToString() |> ignore
    failwith "Unimplemented formatting unexpectedly succeeded"
with :? NotSupportedException -> ()
printfn "Original ACT reference bound to the replacement: %s" actual.FullName

let directory = Directory.CreateTempSubdirectory("bxivact-contract-")
try
    let globals = actual.GetType("Advanced_Combat_Tracker.ActGlobals", true)
    check (isNull (globals.GetField("oFormActMain").GetValue(null)))
    do
        use environment = new Bxivact.ActCompat.HostEnvironment(directory.FullName)
        check (not (isNull (HostProbe.Root())))
        check (HostProbe.Character("Example Character") = "Example Character")
        check (HostProbe.AppDataPath() = directory.FullName)
        check (HostProbe.MissingRegistration())
        HostProbe.BackgroundDiagnostic()
        let count, diagnostic = environment.Diagnostics
        check (count = 1L && diagnostic.Contains("background diagnostic"))
        check (fst environment.Errors = 0L)
        try
            HostProbe.Restart()
            failwith "Embedded RestartACT unexpectedly succeeded"
        with :? PlatformNotSupportedException -> ()
    check (isNull (HostProbe.Root()))
    printfn "Original ACT static field references bound and globals were cleared on disposal."
finally Directory.Delete(directory.FullName, true)
