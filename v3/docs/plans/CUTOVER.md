# Cut-over plan — v3.0

**Proposal.** What has to be true (and documented) before `v3-core` can merge to `main` and ship as
`SealTools v3.0`. The audit behind it is `records/` (the 2026-10-27 docs audit) plus
`PLAN-V3-CORE.md` §10 step 7. A plan is work not yet done; the shipped shape graduates into `spec/`.

## Decisions (made 2026-10-27; ⚑ marks a default chosen for the builder, not the player)

1. **Deliverable is a zip, no installer for v3.0.** ⚑ Plan §13.1 allows Inno as optional; v3.0 ships
   `SealTools-v3.0.zip` with ONE top folder containing the shell exe, its Flutter `data/`,
   `seal-core.exe`, `DirectML.dll`, `models/`, and the config templates. An installer can be a later
   release.
2. **Naming.** The shell binary and window title become `SealTools`/`SealTools.exe` (plan §15), the
   pubspec version `3.0.0+1`, the core `clientVersion` `0.1.0` unchanged (it is the protocol client
   version, not the product version). `root` discovery and `core_path.dart` keep finding
   `seal-core.exe` beside the shell exe.
3. **Config templates ship as examples.** `config/defaults.yaml.example` (shipped base; the core
   accepts `defaults.yaml` or `defaults.yaml.example` with the documented precedence) and a NEW
   `config/local.yaml.example` (the machine/personal half) plus the attribute files. No real
   `local.yaml` in the zip, ever.
4. **Migration is a copy.** A v2 install becomes a v3 install by copying `config/local.yaml`, the
   calibration artifacts beside it (`calib_gem_result.png`, any `calib_*.png`) and the attribute files
   into the v3 folder's `config/`. The formats are unchanged; the guide documents it. No converter.
5. **`docs/pet-data.csv` ships** (the pet feeder reads `<root>/docs/pet-data.csv`; without it the
   feeding estimate is unavailable and the schedule falls back). The publish script copies v2's file.
6. **The unchanged firmware sketch is republished** as `SealTools-v3.0-firmware.zip` (same version;
   no reflash needed for an existing user) rather than linking the v2.16 asset — a fresh download
   should contain everything.
7. **v3.0 ships WITHOUT global hotkeys, and says so.** PLAN §6.5 explicitly makes hotkeys a
   measurement first, not a mechanical port: the core process never owns the foreground window, so a
   core-side `GetAsyncKeyState` poller may be blocked by the anti-cheat while in game. The Stop
   button and the shell's own controls always work. The poller stays a named OPEN item with the
   measurement that decides between core-side polling and shell-side commands.
8. **Capture/hide is a cut-over BLOCKER and gets fixed, not documented around.** `PROTOCOL.md` §4
   says the shell hides itself before asking for a capture; the shell had no hide path, so from the
   UI the game is never foreground and `capture` refuses (`game_not_foreground`). The Flutter runner
   gains a small host method channel (hide/show), the Calibrate tab hides → waits one frame → captures
   → shows, and a widget test pins the order with a fake host.
9. **zh-TW twin: yes.** `USER_GUIDE.zh-TW.md` written from the v3 guide (not by editing v2's), same
   heading structure. Screenshots: new captures of the v3 shell; if none can be taken, the guide
   ships text-only rather than reusing WPF screenshots.
10. **Docs ship in the zip** (`docs/USER_GUIDE.md`, `docs/USER_GUIDE.zh-TW.md`, `docs/INSTALL.md`)
    as well as staying in the repo — the offline user needs them.
11. **COORDINATES is not a separate v3 file.** `CALIBRATION.md` already owns the one-space/two-space
    model, DPI-unawareness, capture, frame→offset and artifacts; a second file would duplicate it. Add
    `CALIBRATION.md` §8 "Portability and migration" and keep v2's COORDINATES as the record; PLAN step
    7's list is amended by this decision.
12. **Merge/tag/publish is the player's call.** The final step (merge to `main`, tag `v3.0`, GitHub
    release) is not performed by this plan's work; it is prepared and verified up to the artifacts.

## Work items (all must be done before the merge)

- [ ] Capture/hide lifecycle (item 8) — shell runner channel + Calibrate flow + tests.
- [ ] Naming/version alignment (`SealTools.exe`, window title, pubspec 3.0.0+1, publish script paths).
- [ ] `USER_GUIDE.md` + `USER_GUIDE.zh-TW.md` + `INSTALL.md` (install, first run, migration, update,
      uninstall, troubleshooting, what is not built in 3.0, the hotkey limitation).
- [ ] Spec corrections: PROTOCOL front matter/§11 status, DESIGN status/§8, CALIBRATION status/§4
      frame cache/§6 Setup wording/§7 counts + new §8, CONFIG stale bullets + dangling pointer,
      TOOL-FLOWS status + per-tool "loop is live" lines, OPEN prune.
- [ ] Folder READMEs (`spec/`, `records/`, `backlog/`), PROGRESS entry, PORT-COVERAGE closure note.
- [ ] Packaging: `v3/publish.ps1` (build → stage → allow-list → zip → firmware zip), config templates,
      `local.yaml.example`, `docs/pet-data.csv`, `DirectML.dll` licensing note (a release input; the
      OCR research record cites it).
- [ ] Release record `records/RELEASE-v3.0.md` + root README v3.0 rewrite + board rows.
- [ ] Live verification: at least one run of each card in the game (the repo's rule: a tool is done
      when it has run, not when it compiles) and one calibrator pass from the shell UI. ⚑ needs the
      player.
