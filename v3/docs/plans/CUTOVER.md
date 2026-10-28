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
   should contain everything. The zip has one top folder, `seal_mouse/`, carrying only the unchanged
   `seal_mouse.ino`; the publish script audits both the sketch directory and the archive.
7. **v3.0 ships WITHOUT global hotkeys, and says so.** PLAN §6.5 explicitly makes hotkeys a
   measurement first, not a mechanical port: the core process never owns the foreground window, so a
   core-side `GetAsyncKeyState` poller may be blocked by the anti-cheat while in game. The Stop
   button and the shell's own controls always work. The poller stays a named OPEN item with the
   measurement that decides between core-side polling and shell-side commands.
8. **CLOSED — capture/hide landed.** `PROTOCOL.md` §4 says the shell hides itself before asking for
   a capture; the shell had no hide path, so from the UI the game was never foreground and `capture`
   refused (`game_not_foreground`). The Windows runner now serves a small `seal/host` channel
   (`hideWindow`/`showWindow`), the Calibrate tab waits the hide's reply, then one frame (with a
   50 ms fallback so a stalled hidden compositor cannot hang the grab), then captures and SHOWS
   unconditionally in a `finally`; a missing host channel is a note, not a failed capture. Widget
   tests pin `hide, capture-sent, show` on success and on refusal.
9. **zh-TW twin: yes.** `USER_GUIDE.zh-TW.md` written from the v3 guide (not by editing v2's), same
   heading structure. Screenshots: new captures of the v3 shell; if none can be taken, the guide
   ships text-only rather than reusing WPF screenshots.
10. **Docs ship in the zip** (`docs/USER_GUIDE.md`, `docs/USER_GUIDE.zh-TW.md`, `docs/INSTALL.md`,
    and `docs/THIRD-PARTY-NOTICES.md` for the DirectML/ONNX/PP-OCR/Flutter/Rust notices) as well as
    staying in the repo — the offline user needs them, and the notices are a licence obligation.
11. **COORDINATES is not a separate v3 file.** `CALIBRATION.md` already owns the one-space/two-space
    model, DPI-unawareness, capture, frame→offset and artifacts; a second file would duplicate it. Add
    `CALIBRATION.md` §8 "Portability and migration" and keep v2's COORDINATES as the record; PLAN step
    7's list is amended by this decision.
12. **Merge/tag/publish is the player's call.** The final step (merge to `main`, tag `v3.0`, GitHub
    release) is not performed by this plan's work; it is prepared and verified up to the artifacts.

## Packaging findings (the first `-Verify` run, 2026-10-27)

The publish script exists and `-Verify` is RED on purpose (exit 1, seven named inputs). Findings and
the decisions they force:

1. **The OCR models live at `v2/models`, not `v3/models`.** The release command passes
   `-ModelsDir v2\models` (the script's default stays `v3\models` so a missing model source is always
   named); `-Stage` copies the three `.onnx` files into the staged `models/`. The models are not
   committed to the v3 tree.
2. **`DirectML.dll` exists only in the debug target today** (the ORT build artifact). `-Stage` builds
   the release core first, so the release target must place its own; `-Verify` names it until then.
3. **The shell bundle has never been built and still carries the scaffold naming** (`BINARY_NAME
   "seal_shell"`, pubspec `1.0.0+1`). Naming is a cut-over work item (decision 2); the script refuses
   to stage a `seal_shell.exe`.
4. **`docs/pet-data.csv` is a byte-identical copy** of `<repo>/v2/docs/pet-data.csv`.
5. **`attributes.us.yaml` ships under the `.example` name** because v3 has no committed
   `attributes.us.yaml.example`; the script stages the committed file that way so an update never
   overwrites a dictionary the player grows in place. `attributes.yaml` (TW) ships as maintained.
6. **CLOSED — the OCR models are found without `SEAL_OCR_MODELS`.** The core used to read that
   variable only and the shell spawns it with no environment, so a shipped `models/` directory would
   have been unread. `seal-ocr` now has one resolver: the env var wins or fails loudly (never
   silently ignored), then `<config root>/models`, then `<seal-core.exe directory>/models`, a
   directory counting only with all three model files, and `ocr_unavailable`'s `detail.message` names
   every directory tried. The shell still spawns with no environment — nothing for packaging to do
   beyond shipping `models/`.

## Staging result (2026-10-27)

`-Stage -ModelsDir v2\models` completed in this checkout after the sandbox interference was worked
around (the teammate session could not run Flutter at all; the Lead session could). Cargo's release
core built in 7 s (incremental after the earlier 16.7 s full build), the Flutter Windows release
bundle built in 30.2 s, and the hardened stage audit passed, producing ONE top folder:

```
dist/SealTools-v3.0/
  SealTools.exe            125,440   (release shell; window title "Seal Tools v3")
  flutter_windows.dll   21,289,472
  data/                    app.so, icudtl.dat, flutter_assets/**
  seal-core.exe        25,491,968
  DirectML.dll         18,527,776
  models/                  the three PP-OCRv4 .onnx files
  config/                  defaults.yaml.example, local.yaml.example, attributes.yaml,
                           attributes.us.yaml.example  (NO real local.yaml)
  docs/                    USER_GUIDE.md, USER_GUIDE.zh-TW.md, INSTALL.md, pet-data.csv
```

`-Verify -ModelsDir v2\models` now exits 0 with "All release sources are present."

****Re-cut (2026-10-27).** The pre-publish conformance audit (see the release record) found shipped
message/doc truth defects and a fixture-coverage gap; they were fixed at source, all gates re-run, and
the zips re-cut. Final artifact: `SealTools-v3.0.zip` 44,811,539 B SHA-256
`990C262906966DBE86AB1FF2667063492A00FF1C715B57DE1A1CD1E7995F483A`; the firmware zip is unchanged (`498E6A4BA1F8552C2BBE8B828AE892B338CC675BA5016CACE0089A3C35CCEF3B`). The first cut's
hashes are superseded and nothing was published.

The release ZIP was also extracted to a fresh temp folder and launched from there: the extracted `SealTools.exe` spawned the extracted `seal-core.exe` and seeded a fresh config, so the artifact is verified as an INSTALL rather than only as a staging folder.

The staged build was launched as a real release (not the debug dev pair) and self-checked:**
`SealTools.exe` resolved `seal-core.exe` BESIDE it (the release rule, no environment), the core
seeded `config/local.yaml` from `local.yaml.example` on first run, and the window came up titled
"Seal Tools v3" -- so the packaged folder is internally consistent before any game is involved. What
it cannot prove without the player is the tools actually driving the game, which is why the live
verification item stays open.

## Publishing checklist (player)

**v3.0.0 and v3.0.1 were superseded before publication.** The first live pass found defects fixed in
v3.0.1, and the second pass corrected the Sell selection to per-run, fixed in v3.0.2 (see
`records/RELEASE-v3.0.2.md`). Use the **v3.0.2 fix-release section at the end of this file** for the
release actually to publish; the checklist below is the original v3.0.0 plan, kept for its reasoning.

The artifacts exist locally; publishing is deliberately not performed by the plan's work. When the
live pass is clean, the release step is mechanical:

1. **Merge and tag** (on the player's machine): merge `v3-core` into `main`, then `git tag v3.0` and
   push the branch and the tag — the push is the player's call, this repository has never pushed from
   the v3 work.
2. **GitHub release** `v3.0` with the two assets:
   - `dist/SealTools-v3.0.zip` — 44,811,539 B,
     SHA-256 `990C262906966DBE86AB1FF2667063492A00FF1C715B57DE1A1CD1E7995F483A`
   - `dist/SealTools-v3.0-firmware.zip` — 5,254 B,
     SHA-256 `498E6A4BA1F8552C2BBE8B828AE892B338CC675BA5016CACE0089A3C35CCEF3B`
   Release body: `v3/docs/records/RELEASE-v3.0.md` (it already states the no-reflash paragraph, the
   not-built list and the DirectML basis). Attach the zips exactly as built; do not re-zip.
3. **Root README**: rewrite for v3.0 and add the version-table row. Draft row (keep the table's existing
   columns):
   `| v3.0 | 2026-10-27 | Rust core + Flutter shell; all eight tools live; 18 calibration marks; per-run logs; replay core. No .NET. Firmware unchanged. |`
   Keep the v2.16 section as older-release history, update the download link to the v3.0 release, and
   leave the firmware link pointing at the v3.0-firmware asset (same sketch).
4. **Archive the plans per the repo rule**: `PLAN-V3-CORE.md` → `v2/docs/archive/` (it lives in v2's
   plans tree) and `PLAN-V3-SHELL.md` → `v3/docs/archive/` (create the folder with a README); the
   cut-over plan moves with them once the release is published. Their durable reasoning has already
   graduated into `spec/`.
5. **Post-publish check**: download both assets from the release, compare SHA-256 with the values
   above, unzip the app and launch it once (the same check done locally), and confirm the firmware zip
   contains only `seal_mouse/seal_mouse.ino`.
6. **do not** move v2's docs into v3 (PLAN §13.5) and do not start a second changelog — the release
   record + root README table are the two homes.

## Work items (all must be done before the merge)

- [ ] OCR model resolution fallback (finding 6) — env var, then root, then exe-relative.
- [ ] Name/version alignment — `SealTools.exe`, window title, pubspec `3.0.0+1`.
- [ ] Capture/hide lifecycle (item 8) — shell runner channel + Calibrate flow + tests.
- [ ] Naming/version alignment (`SealTools.exe`, window title, pubspec 3.0.0+1, publish script paths).
- [x] `USER_GUIDE.md` + `USER_GUIDE.zh-TW.md` + `INSTALL.md` (install, first run, migration, update,
      uninstall, troubleshooting, what is not built in 3.0, the hotkey limitation). Landed text-only;
      `publish.ps1 -Verify` now accepts all three, leaving only the four release-build artifacts red.
- [ ] Spec corrections: PROTOCOL front matter/§11 status, DESIGN status/§8, CALIBRATION status/§4
      frame cache/§6 Setup wording/§7 counts + new §8, CONFIG stale bullets + dangling pointer,
      TOOL-FLOWS status + per-tool "loop is live" lines, OPEN prune.
- [ ] Folder READMEs (`spec/`, `records/`, `backlog/`), PROGRESS entry, PORT-COVERAGE closure note.
- [x] Packaging: `v3/publish.ps1` (build → stage → allow-list → zip → firmware zip; `-Selftest` runs
      the version/deny/post-zip/firmware guards without a build; release command
      `-ModelsDir v2\models`), config templates,
      `local.yaml.example`, `docs/pet-data.csv`, `DirectML.dll` licensing note (a release input; the
      OCR research record cites it). Staged and launched.
- [x] DirectML redistribution acknowledged by the player (2026-10-27); the terms were read at source
      (Microsoft Software License Terms for DirectML, clause 1(a)) and `docs/THIRD-PARTY-NOTICES.md`
      ships as a required release doc. `-Release -ModelsDir v2\models -AcceptDirectMLRedistribution`
      then passed all three audits: `SealTools-v3.0.zip` 44,808,739 B SHA-256
      `B38E673C0CA75892C30AAD6239222582A88ADDDCD58C86CDAD50EC59F529C9CA` (the first cut, superseded the same day by the pre-publish audit fixes — final hashes in
      `records/RELEASE-v3.0.md`: 44,811,539 B, SHA-256 `990C262906966DBE86AB1FF2667063492A00FF1C715B57DE1A1CD1E7995F483A`),
      `SealTools-v3.0-firmware.zip` 5,254 B SHA-256
      `498E6A4BA1F8552C2BBE8B828AE892B338CC675BA5016CACE0089A3C35CCEF3B`. Recorded in
      `records/RELEASE-v3.0.md`.
- [ ] Release record `records/RELEASE-v3.0.md` + root README v3.0 rewrite + board rows.
- [ ] Live verification: at least one run of each card in the game (the repo's rule: a tool is done
      when it has run, not when it compiles) and one calibrator pass from the shell UI. ⚑ needs the
      player.

---

## v3.0.1 fix release (2026-10-28)

The release to publish. The version bump covers the first live pass's fixes, listed in
`records/RELEASE-v3.0.1.md`; the firmware zip is byte-identical to v3.0.0's because the sketch never
changed.

1. **Merge and tag**: merge `v3-core` into `main`, `git tag v3.0.1`, push branch + tag (the push is the
   player's call).
2. **GitHub release** `v3.0.1` with:
   - `dist/SealTools-v3.0.1.zip` — 44,841,808 B,
     SHA-256 `85AAAF9833E53D1AC7E9894E09C4556B2CB514BAA4D5F537BF15BF43AFB08961`
   - `dist/SealTools-v3.0.1-firmware.zip` — 5,254 B,
     SHA-256 `498E6A4BA1F8552C2BBE8B828AE892B338CC675BA5016CACE0089A3C35CCEF3B`
   Release body: `v3/docs/records/RELEASE-v3.0.1.md`. Attach exactly as built; do not re-zip.
3. **Root README**: rewrite for v3.0.1 and add the version-table row:
   `| v3.0.1 | 2026-10-28 | Rust core + Flutter shell; all eight tools live; 18 calibration marks; per-run logs; replay core. No .NET. Firmware unchanged. |`
   Keep the v2.16 section as older history and point downloads/firmware at the v3.0.1 assets.
4. **Archive the plans**: `PLAN-V3-CORE.md` → `v2/docs/archive/`, `PLAN-V3-SHELL.md` and
   `CUTOVER.md` → `v3/docs/archive/` (with a folder README); `PLAN-V3-CALIBRATION.md` stays a live
   proposal until its work ships.
5. **Post-publish check**: download both v3.0.1 assets, compare SHA-256 with the values above, unzip
   the app and launch it once (the same check done locally), and confirm the firmware zip holds only
   `seal_mouse/seal_mouse.ino`.
6. **do not** move v2 docs into v3, and do not start a second changelog — the release records plus the
   root README table are the homes.

---

## v3.0.2 fix release (2026-10-28) — the one to publish

Supersedes v3.0.0 and v3.0.1. The only change from v3.0.1 is the Sell selection: it is now a per-run
`tool.start` argument (memory-only UI), because the player clarified the selection changes every sell
and a saved one can sell the wrong slots. Details and gates: `records/RELEASE-v3.0.2.md`.

1. **Merge and tag**: merge `v3-core` into `main`, `git tag v3.0.2`, push branch + tag (the push is the
   player's call).
2. **GitHub release** `v3.0.2` with:
   - `dist/SealTools-v3.0.2.zip` — 44,838,165 B,
     SHA-256 `7C8D79600D0DE9349B1DF00E18789A91438AB57A77E2E5EE7CD30364BB6A3084`
   - `dist/SealTools-v3.0.2-firmware.zip` — 5,254 B,
     SHA-256 `498E6A4BA1F8552C2BBE8B828AE892B338CC675BA5016CACE0089A3C35CCEF3B`
   Release body: `v3/docs/records/RELEASE-v3.0.2.md`. Attach exactly as built; do not re-zip.
3. **Root README**: rewrite for v3.0.2 and add the version-table row:
   `| v3.0.2 | 2026-10-28 | Rust core + Flutter shell; all eight tools live; 18 calibration marks; per-run logs; replay core; per-run Sell selection. No .NET. Firmware unchanged. |`
   Keep the v2.16 section as older history and point downloads/firmware at the v3.0.2 assets.
4. **Archive the plans**: `PLAN-V3-CORE.md` → `v2/docs/archive/`, `PLAN-V3-SHELL.md` and
   `CUTOVER.md` → `v3/docs/archive/`; `PLAN-V3-CALIBRATION.md` stays a live proposal.
5. **Post-publish check**: download both v3.0.2 assets, compare SHA-256 with the values above, unzip and
   launch the app once, and confirm the firmware zip holds only `seal_mouse/seal_mouse.ino`.
6. **do not** move v2 docs into v3, and do not start a second changelog.
