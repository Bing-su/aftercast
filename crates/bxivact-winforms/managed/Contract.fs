namespace System.Windows.Forms

open System

/// Linux type binding only: constructing controls requires a real UI implementation.
[<AllowNullLiteral>]
type TabPage() =
    do raise (PlatformNotSupportedException("Linux WinForms UI is not implemented; TabPage cannot be constructed"))

/// Supply the Label signature for tested modern-runtime binding; UI creation must fail.
[<AllowNullLiteral>]
type Label() =
    do raise (PlatformNotSupportedException("Linux WinForms UI is not implemented; Label cannot be constructed"))
