# GUI design verification

final result: passed

## Evidence and scope

| Item | Evidence |
| --- | --- |
| Selected visual target | `C:/Users/ks251/.codex/generated_images/01a17ae2-d4f2-7381-bf46-c5ab9c69d30e/exec-90a9e41e-0ce2-44ee-a6bf-9b161de11e9a.png` |
| Implementation | `target/gui-analysis-ko.png`, captured from the actual Windows WebView2 desktop application |
| Full comparison | `target/gui-design-comparison.png` (reference left, implementation right) |
| Focused comparison | `target/gui-design-comparison-detail.png` (navigation, exploration and table) |
| Viewport | 1440 × 1024 CSS pixels, device scale factor 1 |
| Normalization | Reference 1487 × 1058 pixels fitted to 1440 × 1024; implementation 1440 × 1024 unchanged. Comparison is 2880 × 1024. |
| State | Korean, Catppuccin Mocha, selected encounter → Source → outgoing Damage category, graph expanded |
| Additional captures | `target/gui-fixture-compact.png` (850 × 580), `target/gui-settings-light.png` (Latte settings) |

This is a redesign of the existing native application, not a separate web
prototype. Existing Playwright tests attach to its real WebView2 instance.
The source contains illustrative data and capabilities beyond the current GUI;
the implementation uses real synthetic-plugin results rather than copying mock
numbers or adding nonfunctional controls.

## Comparison history

| Pass | Finding | Fix and post-fix evidence |
| --- | --- | --- |
| Initial | P2: table reserved a large empty scroll viewport and duplicated pagination | Used Chorale's inline mode and hid its unlocalized footer. The final capture has natural table height and one pagination strip. |
| Initial | P2: sidebar selection inherited a light-theme surface in Mocha | Used scoped Catppuccin variables directly. Final selected category uses a dark surface and mauve text. |
| Initial | P2: screenshot retained a scrolled main pane, hiding its heading | Reset the pane's scroll position before capture. Final breadcrumb and heading are visible. |
| Refinement | P2: navigation/sidebar/title proportions were too small against the reference | Increased the top bar to 64px, desktop sidebar to 330px, title to 28px and desktop tabs to 16px. Rechecked the full and focused combined images. |
| Review | Native details/summary did not match the requested disclosure style | Replaced explorer/chart disclosure with a shared themed accordion. Verified Enter/Space, expanded/control attributes, hidden content, retained state and reduced motion in the real desktop app. |
| Review | P1: navigating away from the import pane canceled its task and left the phase stuck | Reproduced with 50,000 fixture lines, moved import lifetime to the application and captured the locale context. The same navigation test now completes with an enabled import action. |
| Final | No remaining actionable P0/P1/P2 layout or interaction findings in the implemented scope | Final Korean, compact and light captures, plus passing desktop interaction checks. |

## Required visual surfaces

| Surface | Assessment |
| --- | --- |
| Fonts and typography | Segoe UI/Malgun Gothic; readable 14px UI, 16px desktop workflow tabs and 28px analysis title. Numeric table columns are right-aligned. |
| Spacing and layout | Full-width top tabs, contextual left sidebar, table before collapsible chart, persistent footer. Compact layout keeps navigation and footer visible and lets wide tables scroll horizontally. |
| Colors and tokens | Existing Catppuccin Mocha/Latte system; mauve selection and muted secondary text. Flat surfaces without generated gradients or decorative cards. |
| Images and icons | No raster/brand assets required. Existing Lucide icon library and Dioxus chart renderer supply UI icons and the chart. No invented logos or game-specific imagery. |
| Copy and content | Korean/English locale coverage retained. Plugin-owned names/categories/directions are preserved. Overlapping ACT aggregate rows are explicitly not summed. |

## Intentional product constraints

| Concept detail | Current application behavior |
| --- | --- |
| Seven illustrative attacks | Real fixture produces two rows for the selected category, including the ACT `All` aggregate. No filler rows or fabricated totals. |
| Average, maximum, critical rate | Displays the fields supplied by `AttackSummary`: swings, total, hits and critical hits, plus category/direction. Maximum is not available in that contract. |
| Time-series chart | Existing damage-contribution overview is retained and accurately labeled; no time samples are invented from aggregate totals. |
| Events/source-log subviews | Omitted until the GUI has those query contracts; the aggregate screen explains their absence. |
| Trigger/history tabs | Visible, disabled and marked as planned. Import works; export is explicitly unavailable. |
| Timer/mini-window utilities | Not implemented or shown as fake controls. Existing host start/stop, import and quit actions remain available. Native OS chrome is retained. |
| Source log timestamps | No fabricated source filename/event timestamp. Footer shows actual host state and operation result. |

These constraints are documented in `crates/aftercast-app/README.md`; this
verification passes the implemented redesign, not every planned ACT feature.

## Functional verification

| Check | Result |
| --- | --- |
| Desktop workflows | 5 passing tests: locale/theme/config persistence, invalid config protection, navigation during save/reload, navigation during import, real Host import/drilldown/restart |
| New interactions | Encounter search; participant/category drilldown; category direction filtering; graph collapse; end combat; disabled planned tabs; compact shell visibility |
| Runtime errors | Page errors and console errors checked; none in the Host workflow. Missing favicon request fixed with an empty icon. |
| Rust | 22 app/unit/bundle/CLI tests passing; includes category/direction filtering and stale-selection loading protection |
| Static validation | Build, formatting, Clippy with warnings denied, and whitespace validation |

## Follow-up polish and limits

- P3: Real parser category names may remain in their parser's language, by design.
- Live game integrations, Linux GUI, high-DPI native capture and very large real
  encounters were not exercised in this change.
- No deployment was performed.

## Implementation checklist

- [x] Remove branding and move workflow navigation to the top.
- [x] Connect the exploration tree to real participant/category aggregates.
- [x] Keep tables primary and chart collapsible.
- [x] Preserve import progress/cancellation and host/config lifecycle behavior.
- [x] Compare final rendered screens and verify key interactions.
