# App-local table and mini parse appearance

Pinned ACT 3.8.5.288 IL, checked by `test:original`'s report-export policy,
shows `Options_ColorUserInterface.fccDataGrid` setting the data grid's base
foreground, background and font, while `Options_MiniParse.fccMiniParse` owns
its window font and colors. The first local slice saves optional grid and mini
text/background colors (`#RRGGBB`), a generic CSS family and an 8–32 px size.
Blank values use the existing theme and migrate old settings without visual
change. Choices affect app views only: no OS setting or font is installed.
Invalid drafts are never interpolated as CSS and cannot be saved.

In a normal grid cell, a nontransparent plugin foreground/background takes
precedence over the corresponding base color; the other component falls back
independently. Error cells retain the semantic error style. For combatant rows,
ACT uses `GetAllies().Contains` to override the **first visible data cell**
foreground and compares the combatant name to `ActGlobals.charName` to override
the personal background on each data cell, before consulting that component's
plugin color. The owned table snapshot now carries both classifications, uses
ACT's name equality for ally membership and rejects classification changes during
callbacks. The defaults match original ForestGreen (`#228b22`) and RGB
`#c8ffc8`; clearing either override disables that emphasis in aftercast. Old
settings that lack the fields migrate to the ACT defaults. Native ACT applies `ToAdjustedColor` to plugin colors. The app now preserves
its HSL adjustment and optional luminosity inversion, with alpha unchanged;
preferences default to zero offsets and inversion off, and only callback colors
are adjusted after the worker returns them. ACT source colors, including the ally
and personal overrides, remain unchanged. The pinned original function validates
1,024 deterministic synthetic cases. The worker exposes ACT's nested
`ActColorSettings` scalar getters (`InvertColors`, `LuminosityOffset`,
`SaturationOffset`) and their corresponding EventHandler add/remove accessors.
The existing ally and personal grid colors are also exposed as ACT `ColorControl`
objects with `ForeColorSetting` and its Color-valued change event. An empty
app color maps to `Color.Empty` as an aftercast-specific disabled highlight.
The headless CheckBox and NumericUpDown senders run on the plugin owner thread;
unchanged values do not fire. Saved app values apply at worker startup and after
a successful settings save. Subscriber exceptions propagate and can leave the
sequential worker update partially applied; the save is retained and the app
reports the runtime failure. Plugin runtime grid colors and adjustments are polled on the owner thread and applied to the table without changing or persisting the user’s settings draft. A conflict notice appears when the draft differs. Old-session and pre-save replies are rejected; a later successful settings save reapplies the saved app values. Polling occurs once per second while the session is ready, so updates are not instantaneous.

The remaining original ColorControl getters are available with their original defaults: `TreeViewEncounterColor1/2/3`, `SpellTimerForeColor`, `SpellTimerBackColor`, `SpellTimerWarningColor`, `SpellTimerExpireColor`, `CustomTriggerActiveBackColor`, `CustomTriggerInactiveBackColor`, and `CustomTriggerCategoryRestrictionForeColor`. They retain their independent Color-valued change events. The app does not yet render all corresponding native ACT surfaces, so changing these controls does not imply a visible effect there. All `FontColorControl` properties/events remain unavailable. The original `FontSetting` signature uses
`System.Drawing.Font` from System.Drawing.Common, which the project does not
currently reference; a local compilation probe fails with CS1069. Adding that
dependency and deciding Linux native-font semantics remain separate work. No font
or OS setting was installed or changed. Native font
measurement, arbitrary installed-family selection, mini text column alignment,
and visual/native Windows behavior remain unverified.

The embedded mini text and separate mini window use the same app-local style;
the window receives updates through its existing session-safe frame. Native
ACT pixel rendering, opacity and click-through are outside this slice.
