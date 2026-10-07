# seal-tools — agent instructions

Windows-only game-automation suite for Seal Online: **C# / .NET 8 WPF**
(`net8.0-windows`), driving an Arduino Pro Micro over a COM port, with OCR. Ships as one
self-contained `.exe`.

`v2/` is the current build. `v1/` is the original Python build — untouched, still working,
and **not where new work goes** unless I say so.

## Commands

From the repository root, the same three commands CI runs:

    dotnet restore v2/SealTools.sln
    dotnet build   v2/SealTools.sln --no-restore --nologo
    dotnet test    v2/SealTools.Tests/SealTools.Tests.csproj --no-build --nologo

`dotnet build` is the gate. `v2/Directory.Build.props` sets `TreatWarningsAsErrors`,
`AnalysisLevel=latest` and `AnalysisMode=Recommended`, so **an unused symbol or an
unreachable branch fails the build**. That is intentional: fix the cause, never silence it
with `#pragma` or by adding to `NoWarn`.

## Layout

| Path | Holds |
|---|---|
| `v2/SealTools.sln` | The solution — 9 projects |
| `v2/SealTools.Core` | Shared: capture, OCR, config, input arbitration |
| `v2/SealTools.Launcher` | The WPF launcher window (card UI) |
| `v2/SealTools.{Tuner,GemComposer,Spammer,Shop,Pet,Quest}` | One project per tool |
| `v2/SealTools.Tests` | Test suite — 24 test files |
| `v2/config` | `defaults.yaml` (committed) and `local.yaml` (**my personal state**) |
| `v2/docs` | All documentation — see below |
| `arduino/` | Firmware sketch, published as a separate release asset |
| `v1/` | Legacy Python build. Leave it alone. |

## Rules

- **`local.yaml` is personal state, not source.** It holds my calibrated presets, is large
  and machine-specific. Never commit it, never regenerate or "clean" it, and never copy
  values out of it into `defaults.yaml` or an `.example` file. The `*.backup-*` and
  `*.corrupt-backup` files beside it are runtime artifacts — leave them alone.
- **Version history lives in exactly one place.** The per-release table is in the root
  `README.md`; the dated reasoning is in `v2/docs/PROGRESS.md`. `v2/README.md` deliberately
  does not repeat it — it went eight releases stale when it did. A new release updates those
  two, and does not start a third copy.
- **`v2/docs/` is the specification.** `TOOL-FLOWS.md`, `DESIGN.md`, `CONFIG.md`,
  `COORDINATES.md` and `USER_GUIDE.md` describe how the tools are meant to behave. Read the
  relevant one before changing behaviour, and update it in the same commit. `PLAN-*.md` are
  design records: don't rewrite them to match a change, add a new one.
- **Firmware and app are versioned together.** If a change needs a firmware change,
  `FirmwareSketchTests` and `FirmwareVersionTests` are the guard — run them and say so in the
  release notes, because a user has to reflash for it to take effect.
- **Never commit build output.** `v2/publish.bat` produces ~144 MB and releases are GitHub
  Release assets, not repo content. Same for `bin/`, `obj/`, `dist/`, `logs/`, `.scratch/`.
- **Windows-only is intentional.** Don't add cross-platform abstractions or propose Linux CI.
- **`.claude/skills/`** holds a Claude Code skill. DSH doesn't read that path, so anything
  DSH must follow belongs in this file instead.

## Style

- Match the existing style. This codebase is consistent and its comments explain *why*, not
  *what* — keep that.
- Documentation is English; `USER_GUIDE.md` has a `USER_GUIDE.zh-TW.md` twin. User-facing
  changes update both.
