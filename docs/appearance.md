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
settings that lack the fields migrate to the ACT defaults. Native ACT applies
`ToAdjustedColor` to plugin colors; the app currently displays raw callback
colors, so luminosity/saturation transformation remains incomplete. Native font
measurement, arbitrary installed-family selection, mini text column alignment,
and visual/native Windows behavior remain unverified.

The embedded mini text and separate mini window use the same app-local style;
the window receives updates through its existing session-safe frame. Native
ACT pixel rendering, opacity and click-through are outside this slice.
