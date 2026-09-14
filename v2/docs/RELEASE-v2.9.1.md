# v2.9.1 Buy / Sell (hot fix)

A hot fix on v2.9, from setting it up on a second PC. Same tools; the layout now works on a screen
that could not show both the tool cards and a capture canvas at once.

## What changed

**`Tools` hides the tool cards.** On a small screen, maximizing the launcher to drag a capture region
left the five cards taking a third of the height the canvas needed. The button sits left of
`Configuration` because it controls the row above it. Hiding the cards opens Configuration at the
same time, and collapsing Configuration brings them back — the two move together so the window is
never left showing neither.

**Three layout faults on the Spammer tab**, all found by looking at a narrow window rather than by
any test:

- **Save Preset** sat *above* the prompt and the cards it saves — and with the editor collapsed, which
  is how the tab opens, it was a lone button with nothing above it.
- **Delete** was clipped mid-word. The row was a horizontal `StackPanel`, which measures its children
  with unbounded width, so the row ran past the card edge.
- The key row's **Fast** tick box drew with no right border, reading as a `C`. Its column was a couple
  of pixels too narrow for the glyph.

**One wrong instruction removed.** The Buy / Sell calibration hint still described marking two shop
rows — a design replaced by a single dragged list region.

## Assets

| File | |
|---|---|
| `SealTools-v2.9.1.zip` | The app. Self-contained — no .NET or Python needed. Template config only; `config\local.yaml` is created on first run. |
| `SealTools-v2.9.1-firmware.zip` | The Arduino sketch. Unzip and open `seal_mouse\seal_mouse.ino` in the Arduino IDE. Required only if your board is not already flashed. |

Installing, including flashing the board, is in `v2/docs/INSTALL.md`. Tools and every button are in
`v2/docs/USER_GUIDE.md`.

## Upgrading from v2.9

Unzip over the old folder **except for `config\`** — your calibration, presets and window placement
live there and in nothing else. Or unzip somewhere new and copy `config\local.yaml` across.

## Why this replaces v2.9 rather than sitting beside it

The v2.9 release is removed so there is a single download, and so the zip matches the tag it is
published under. Replacing the file *inside* v2.9 was the alternative and was rejected: the `v2.9` tag
would then name a commit that is not the build in that zip. The tag still exists, so `git show v2.9`
works, and v2.3 remains available as the last build before buy/sell.

Full history: the [version table](https://github.com/keikeiiu/seal-tools#version-history) and
`v2/docs/PROGRESS.md`.
