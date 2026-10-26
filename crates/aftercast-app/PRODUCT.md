# aftercast desktop

| Context          | Decision                                                                                                                                                     |
| ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Users            | ACT users configuring parser plugins and inspecting combat results                                                                                           |
| Platform         | Windows Dioxus/WebView2, minimum 850 × 580; Linux runtime bundling supported, GUI validation pending                                                         |
| Primary task     | Configure plugins, start the host, import a log, inspect retained encounter aggregates                                                                       |
| First increment  | Analysis, ordered plugin configuration, appearance/runtime settings, diagnostics                                                                             |
| Language         | Korean and English from the first screen                                                                                                                     |
| Visual direction | Task-oriented desktop workspace; official Catppuccin palette, Mocha by default                                                                               |
| Storage          | `config.toml`, logs, ACT state and bundled runtime beside the executable                                                                                     |
| Future work      | Existing `docs/gui-plan.md` remains the implementation roadmap; native plugin windows, overlays, detailed attack queries and live controls follow separately |

Application components use Rust APIs and Tailwind v4 utilities. Application-owned
JavaScript bridges are excluded. Slow file, clipboard and host operations must
leave the UI responsive, with explicit failure and cancellation states.
