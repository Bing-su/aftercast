namespace Bxivact.ActCompat

open System.Threading

/// Identifies a fresh host lifetime; this is not yet the original ACT object model.
[<Sealed>]
type HostSession() =
    static let mutable nextId = 0L
    let id = uint64 (Interlocked.Increment(&nextId))
    member _.Id = id
