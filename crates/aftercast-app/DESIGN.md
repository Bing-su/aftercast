---
name: aftercast desktop
description: A focused Windows workspace for plugins and encounter results.
colors:
  primary: "var(--catppuccin-color-mauve)"
  background: "var(--catppuccin-color-base)"
  foreground: "var(--catppuccin-color-text)"
  muted-foreground: "var(--catppuccin-color-subtext1)"
  border: "var(--catppuccin-color-surface1)"
  accent: "var(--catppuccin-color-surface0)"
  mantle: "var(--catppuccin-color-mantle)"
  error: "var(--catppuccin-color-red)"
typography:
  headline:
    fontFamily: '"Segoe UI", "Malgun Gothic", sans-serif'
    fontSize: "24px"
    fontWeight: 600
    lineHeight: "32px"
    letterSpacing: "-0.025em"
  title:
    fontFamily: '"Segoe UI", "Malgun Gothic", sans-serif'
    fontSize: "18px"
    fontWeight: 600
    lineHeight: "28px"
  body:
    fontFamily: '"Segoe UI", "Malgun Gothic", sans-serif'
    fontSize: "14px"
    fontWeight: 400
  label:
    fontFamily: '"Segoe UI", "Malgun Gothic", sans-serif'
    fontSize: "12px"
    fontWeight: 400
    lineHeight: "16px"
  field-label:
    fontFamily: '"Segoe UI", "Malgun Gothic", sans-serif'
    fontSize: "14px"
    fontWeight: 500
    lineHeight: "1"
rounded:
  md: "6px"
  full: "calc(infinity * 1px)"
spacing:
  2: "8px"
  3: "12px"
  4: "16px"
  5: "20px"
  6: "24px"
  7: "28px"
  8: "32px"
components:
  button-primary:
    backgroundColor: "{colors.primary}"
    textColor: "{colors.background}"
    rounded: "{rounded.md}"
    height: "36px"
    padding: "0 12px"
  button-outline:
    backgroundColor: "{colors.background}"
    textColor: "{colors.foreground}"
    rounded: "{rounded.md}"
    height: "36px"
    padding: "0 12px"
  button-ghost:
    textColor: "{colors.muted-foreground}"
    rounded: "{rounded.md}"
    height: "36px"
    padding: "0 12px"
  input:
    backgroundColor: "{colors.background}"
    textColor: "{colors.foreground}"
    rounded: "{rounded.md}"
    height: "36px"
    padding: "0 12px"
  navigation-item:
    textColor: "{colors.muted-foreground}"
    rounded: "{rounded.md}"
    padding: "10px 12px"
  encounter-item:
    padding: "12px 20px"
  summary-strip:
    padding: "20px 0"
  field-label:
    typography: "{typography.field-label}"
  separator:
    backgroundColor: "{colors.border}"
    height: "1px"
    width: "100%"
  badge-outline:
    textColor: "{colors.foreground}"
    rounded: "{rounded.md}"
    padding: "2px 10px"
  progress:
    backgroundColor: "{colors.accent}"
    rounded: "{rounded.full}"
    height: "8px"
    width: "100%"
---

# Design System: aftercast desktop

## Overview

**Creative North Star: "The Encounter Workspace"**

aftercast is a task-focused Windows desktop workspace. Its visual character comes from the official Catppuccin palette, readable system typography, and flat, divided work areas. Mocha is the default; Macchiato, Frappé, and Latte retain the same semantic roles and component geometry.

The interface gives most space to the current task while keeping ordered navigation, encounters, host actions, and status available around it. Korean and English use the same quiet typographic hierarchy. The existing Rust components and Tailwind v4 utilities define the system; this document captures that first implemented increment.

**Key Characteristics:**

- Official Catppuccin colours with a restrained mauve action accent.
- System fonts for English and Korean desktop reading.
- Flat surfaces divided by thin borders and tonal changes.
- Compact controls, spacious task content, and persistent status.

## Colors

The palette pairs Catppuccin's soft mauve with a layered neutral workspace. Frontmatter preserves the live CSS variable bindings from `tailwind.css`; the installed Catppuccin package owns flavour values.

### Primary

| Token     | Character        | Observed use                                                                                                     |
| --------- | ---------------- | ---------------------------------------------------------------------------------------------------------------- |
| `primary` | Catppuccin Mauve | Primary buttons, active navigation icons, selected encounter text, chart bars, keyboard focus and text selection |

### Neutral

| Token              | Character            | Observed use                                                     |
| ------------------ | -------------------- | ---------------------------------------------------------------- |
| `background`       | Catppuccin Base      | Main canvas, inputs, outline buttons and primary-button text     |
| `foreground`       | Catppuccin Text      | Titles, body copy and exact result values                        |
| `muted-foreground` | Catppuccin Subtext 1 | Help text, metadata, chart labels and resting secondary controls |
| `border`           | Catppuccin Surface 1 | Structural dividers, field outlines and table grid               |
| `accent`           | Catppuccin Surface 0 | Navigation selection, encounter selection and hover surfaces     |
| `mantle`           | Catppuccin Mantle    | Sidebar, footer, table header and table controls                 |

Catppuccin Red (`error`) communicates errors in the alert strip with a translucent red background. It is a state colour rather than another branding accent.

**The Flavour Binding Rule.** Resolve colours through the current Catppuccin flavour; a new screen must change with the existing appearance selector.

## Typography

**Body and heading font:** Segoe UI, followed by Malgun Gothic and the platform sans-serif fallback.

**Character:** A single system-font stack keeps the workspace familiar and supports Korean alongside English. Weight and spacing establish hierarchy; numeric summaries and tables use tabular figures.

| Role     | Application                                                     |
| -------- | --------------------------------------------------------------- |
| Headline | Screen heading; semibold with tight tracking                    |
| Title    | Encounter heading; semibold                                     |
| Body     | General interface text and navigation                           |
| Label    | Metadata, chart captions, encounter subtitles and footer status |

Screen descriptions use an intermediate size (13px), generous leading (28px), and a reading width up to (68ch). The shared field Label defaults to medium weight (14px); settings and log-path labels override its size to (13px), and plugin-type labels use muted text (12px). Settings subsection headings are semibold (15px). Summary values use medium weight (18px). The wordmark is semibold (19px), and chart axis labels use (11px).

## Layout

The window is a full-height flex column: host-action header, a flexible two-column workspace, and a status footer. The sidebar and main content scroll independently. The desktop minimum is (850 × 580px).

| Region       | Compact desktop                                  | At widths of 1000px and above                          |
| ------------ | ------------------------------------------------ | ------------------------------------------------------ |
| Sidebar      | Fixed (210px)                                    | Fixed (250px)                                          |
| Main content | Flexible; padding (24px)                         | Horizontal padding (32px), vertical padding (28px)     |
| Header       | Minimum height (68px), horizontal padding (16px) | Horizontal padding (24px); host status becomes visible |
| Footer       | Minimum height (36px), horizontal padding (20px) | Same                                                   |

Settings content is capped at (768px); analysis fills the available content width. Native selects are (260px) wide and shrink within their container. Fields fill their available width. Repeated spacing follows a (4px) base; the reused steps are recorded in frontmatter.

Navigation appears above the encounter list. Encounter rows preserve their source order, and the scrolling list sits above a local-storage note. Main content begins with a heading and description; related settings are separated by border rules. The analysis summary uses a horizontal strip followed by a chart and a data table. This is the observed first feature screen, not a requirement to add charts or summary strips to other screens.

## Elevation & Depth

The current interface uses no decorative shadows. Base and Mantle distinguish working regions; Surface 0 marks hover and selection; Surface 1 provides thin boundaries. Focus is an outline, not a shadow. Settings are divided sections rather than floating cards.

**The Flat Workspace Rule.** Use the existing tonal surfaces and borders to separate adjacent work areas.

## Shapes

Controls, navigation items and outline badges have gently curved corners (6px). Structural regions, encounter rows, summary strips and the import-progress panel retain square edges. The progress track is fully rounded. Borders and horizontal separators are thin (1px). Small status indicators are circular; line icons are usually (16px), with a larger app mark (20px).

## Components

### Buttons

Compact, clear actions with a shared Rust `Button` API.

| Variant/state  | Treatment                                                                                     |
| -------------- | --------------------------------------------------------------------------------------------- |
| Primary        | Mauve fill, Base text; start host and save settings are examples                              |
| Outline        | Base fill, Text foreground and Surface 1 border; import, stop, reload and cancel are examples |
| Ghost          | Transparent resting surface and Subtext 1 foreground; utility actions are examples            |
| Hover          | Primary uses brightness (95%); outline and ghost use Surface 0, with ghost text becoming Text |
| Keyboard focus | Mauve outline (2px) offset by (2px)                                                           |
| Disabled       | Opacity (45%) and pointer interaction suppressed                                              |

Buttons share height, radius and padding from frontmatter, medium text weight (500), icon gap (8px), and icon size (16px). Colour transitions use Tailwind's default duration (150ms) and easing; reduced motion disables the transition. Labels remain sentence case.

### Inputs / Fields

The shared Rust `Input` presents editable paths as full-width fields. It uses a thin Surface 1 border, Base fill, Text foreground and Subtext 1 placeholders. It shares button geometry and keyboard outline treatment. Disabled fields use opacity (45%). Native selects match the field height and radius, with horizontal padding (10px). Every field has an associated visible label.

### Labels and Separators

The shared `Label` requires a field association through `html_for`. Its default is medium system text (14px) with compact leading (1), an inline gap (8px), and suppressed text selection. Current settings/log fields use vertical margins (16px above, 8px below) and smaller text (13px); plugin labels use muted text (12px) and a top margin (12px). The component exposes peer-disabled styling, though the current label-before-field arrangement does not activate that state.

The shared `Separator` is a full-width, nonshrinking horizontal rule in Surface 1 (1px high), with `role="separator"` and horizontal orientation. Settings use vertical margins (24px) between Appearance, Runtime and the action row.

### Status Badges

The shared `Badge` is a noninteractive outline: transparent fill, Surface 1 border, Text foreground, small text (12px), rounded corners, and compact padding from frontmatter. It defaults to semibold (600); current host and plugin status callers use normal weight (400). Host status also uses muted text and a circular indicator (6px): Mauve while ready, muted otherwise. The host badge appears only at the wide-desktop breakpoint. Plugin status badges sit below the plugin path with a top margin (8px).

### Progress

The shared `Progress` is a custom, named `role="progressbar"` with a Surface 0 track (8px high) and Mauve fill. It bounds the maximum to at least one, clamps the value to that maximum, and exposes minimum, maximum and current values through ARIA. The full-width fill translates horizontally to represent completion; transform changes use a transition (300ms), disabled for reduced motion. Import places the track below the status/cancel row with a top margin (16px), followed by exact byte and record counts.

### Navigation

Full-width icon-and-label buttons use rounded corners, muted resting text and Surface 0 hover. The current page uses Surface 0, Text foreground and a Mauve icon, identified by `aria-current="page"`. Sidebar navigation remains visible at the minimum desktop size; the header's secondary host-status label is hidden below the breakpoint.

### Encounter Rows

Flat, full-width buttons use a bottom divider, a medium-weight truncated title, and a muted smaller zone subtitle. Selection uses Surface 0 and Mauve title text, exposed through `aria-pressed`. The title and zone remain separate lines.

### Summary and Data Table

The summary strip uses a top rule, muted labels and tabular medium-weight values, with horizontal gaps (36px). The participant table maps Chorale's public theme variables to Catppuccin: Mantle headers/toolbars, Base rows, Surface 1 borders, Surface 0 hover/selection, and Mauve accent. Numeric values retain tabular figures. The table container permits scrolling rather than compressing columns beyond readability.

### Status and Feedback

The footer keeps a single-line truncated status at the left, with diagnostics, application-folder access and version at the right. Error feedback uses an alert strip; import progress uses a bordered Mantle panel containing the shared bounded, named ARIA progressbar and an outline cancel action. These state treatments reuse the same typography and controls.

## Do's and Don'ts

### Do:

- **Do** use the live Catppuccin semantic colours across all four flavours.
- **Do** reuse the Rust Button and Input components for matching controls.
- **Do** retain visible field labels, keyboard focus and explicit selected states.
- **Do** use tabular figures for result metrics and preserve readable table scrolling.
- **Do** verify Korean and English at the minimum desktop window size.

### Don't:

- **Don't** hard-code Mocha colours into a screen that supports flavour switching.
- **Don't** replace the system-font stack with a decorative display face.
- **Don't** add shadows or floating-card styling to the existing flat workspace.
- **Don't** use colour alone to identify the current page or selected encounter.
