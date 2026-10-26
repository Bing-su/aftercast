# Adapted UI sources

| Local source                  | Upstream                                                                                                                                 | Revision                                   | Local adaptation                                                                                          |
| ----------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------ | --------------------------------------------------------------------------------------------------------- |
| `src/components/button.rs`    | [Rust/UI button](https://github.com/rust-ui/ui/blob/1bf779a7b08b778f9afa9ae11ae2b824479c7437/app_crates/registry/src/ui/button.rs)       | `1bf779a7b08b778f9afa9ae11ae2b824479c7437` | Keep desktop button behavior, used variants and focus/disabled styles; omit router links and unused props |
| `src/components/input.rs`     | [Rust/UI input](https://github.com/rust-ui/ui/blob/1bf779a7b08b778f9afa9ae11ae2b824479c7437/app_crates/registry/src/ui/input.rs)         | Same                                       | Keep text input behavior and Tailwind styles; omit unused input-type enum and props                       |
| `src/components/label.rs`     | [Rust/UI label](https://github.com/rust-ui/ui/blob/1bf779a7b08b778f9afa9ae11ae2b824479c7437/app_crates/registry/src/ui/label.rs)         | Same                                       | Require explicit field association; preserve Tailwind merging and disabled peer styling                   |
| `src/components/separator.rs` | [Rust/UI separator](https://github.com/rust-ui/ui/blob/1bf779a7b08b778f9afa9ae11ae2b824479c7437/app_crates/registry/src/ui/separator.rs) | Same                                       | Horizontal settings-group separator with explicit ARIA orientation                                        |
| `src/components/badge.rs`     | [Rust/UI badge](https://github.com/rust-ui/ui/blob/1bf779a7b08b778f9afa9ae11ae2b824479c7437/app_crates/registry/src/ui/badge.rs)         | Same                                       | Keep the used outline variant for host/plugin status; use existing Catppuccin semantic tokens             |
| `src/components/progress.rs`  | [Rust/UI progress](https://github.com/rust-ui/ui/blob/1bf779a7b08b778f9afa9ae11ae2b824479c7437/app_crates/registry/src/ui/progress.rs)   | Same                                       | Byte-count props, clamped bounds, accessible name, Catppuccin tokens and reduced-motion support           |

These sources are licensed under the upstream MIT license in `RUST_UI_LICENSE`.
Cargo dependencies retain their own licenses and notices, including Lucide icons.

The stylesheet imports the official MIT-licensed
[`@catppuccin/tailwindcss`](https://github.com/catppuccin/tailwindcss) package,
locked in `bun.lock`. Its `mocha.css` exposes all four flavour classes;
the app's semantic Tailwind tokens reference those palette variables directly.
