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
independently. Error cells retain the semantic error style. ACT additionally
uses `GetAllies` to override the foreground for ally combatants and compares the
combatant name to `ActGlobals.charName` to override the personal background,
before consulting that component's plugin color. Its plugin colors are also
passed through `ToAdjustedColor`. The first base-color slice does not yet
implement these ally/self overrides or ACT luminosity/saturation transformation;
it must not be described as full color-rule parity. Native font measurement,
mini text column alignment, exact installed-family selection, and visual/native
Windows behavior remain unverified.

The embedded mini text and separate mini window use the same app-local style;
the window receives updates through its existing session-safe frame. Native
ACT pixel rendering, opacity and click-through are outside this slice.
