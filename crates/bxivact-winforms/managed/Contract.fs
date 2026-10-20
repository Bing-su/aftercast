namespace System.Windows.Forms

open System
open System.ComponentModel

/// Store status text and disposal state without fabricating a native window or message loop.
[<AllowNullLiteral>]
type Control() =
    inherit Component()
    let mutable disposed = false
    let mutable text = ""
    member val Name = "" with get, set
    abstract Text: string with get, set
    default _.Text with get () = text and set value = text <- if isNull value then "" else value
    member _.IsDisposed = disposed
    member _.Handle: nativeint = raise (PlatformNotSupportedException("Linux WinForms native handles are not implemented"))
    member _.CreateControl() : unit = raise (PlatformNotSupportedException("Linux WinForms rendering is not implemented"))
    override this.Dispose(disposing) =
        if not disposed then
            disposed <- true
            base.Dispose(disposing)

[<AllowNullLiteral>]
type ScrollableControl() = inherit Control()

[<AllowNullLiteral>]
type Panel() = inherit ScrollableControl()

[<AllowNullLiteral>]
type TabPage() = inherit Panel()

[<AllowNullLiteral>]
type Label() = inherit Control()

[<AllowNullLiteral>]
type ContainerControl() = inherit ScrollableControl()

/// Let plugin settings types resolve their base class without claiming a rendered settings panel.
[<AllowNullLiteral>]
type UserControl() = inherit ContainerControl()

[<AllowNullLiteral>]
type Form() = inherit ContainerControl()
