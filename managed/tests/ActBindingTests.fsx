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
