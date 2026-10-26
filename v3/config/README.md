# v3 config — the dev tree and the shipped install tree

The core takes a **root** whose `config/` subdirectory holds its files:

```powershell
seal-core.exe --root <repo>/v3
```

The root is the directory *containing* `config/` (a packaged install has the shell exe and
`seal-core.exe` at the top of the folder, beside `config/`).

## The two trees

This directory is the **development/install config tree**: committed defaults, dictionaries, the
seed template, and a gitignored `local.yaml` this machine has written. The packaged install tree is
the same shape, built by `v3/publish.ps1`, with one rule: **the only files it carries are templates
and release-maintained dictionaries — never a real `local.yaml`**.

| file | dev tree | shipped install | written by |
|---|---|---|---|
| `defaults.yaml` | committed source base | — (published under the template name) | nobody |
| `defaults.yaml.example` | — | committed `defaults.yaml`, renamed on staging | nobody |
| `local.yaml` | gitignored machine state | **never shipped** | the core (all saves) |
| `local.yaml.example` | committed seed | committed seed | nobody |
| `attributes.yaml` (TW dictionary) | committed | committed | release-maintained by hand |
| `attributes.us.yaml` | committed dev copy (empty today) | — (published under the template name) | the player grows it in place |
| `attributes.us.yaml.example` | — | committed `attributes.us.yaml`, renamed on staging | nobody |

## Shipped-base precedence (`defaults.yaml.example` vs `defaults.yaml`)

The core's load rule, in one line:

1. **`config/defaults.yaml.example` is the base when it exists.** That is what a released install
   ships, so extracting an update replaces the *template* and never the player's own file.
2. Otherwise the tracked **`config/defaults.yaml`** is the base. That is a git checkout/dev tree.
3. If both exist, `defaults.yaml` beside the template is the **legacy layer**: it is read once, its
   differing portable values are folded into `local.yaml`'s `settings:` block, and it is then kept
   as the rollback copy (never written, moved or deleted). This is v2's migration path.

The publish script stages the tracked `defaults.yaml` as `config/defaults.yaml.example`, so a fresh
install reads the template and an upgrade cannot overwrite anything the player saved.

## First run, and what a fresh install starts with

`config/local.yaml` is seeded **only when absent**, from `config/local.yaml.example`, through the
same atomic writer as every other save. The seed is deliberately uncalibrated: the calibrator writes
`buy_sell`/`gem`/`tuner`/`pet`/`tooltip`/`quest` and the shell writes `ui`, so every tool refuses
with its own calibration line until the player calibrates it. A v2 install migrates by copying
`local.yaml` and the `calib_*.png` crops into this directory — the formats are unchanged.

Missing files are loud, not silent: no `local.yaml` and no seed is a boot refusal; a variant's
missing dictionary is refused unless an `<name>.example` template can seed it.

## `docs/pet-data.csv`

The pet feeder reads `<root>/docs/pet-data.csv` (the feeding table used to estimate pet-minutes).
The released file is copied **verbatim** from `v2/docs/pet-data.csv`; it is data, not config, and the
copy lives at `v3/docs/pet-data.csv`. Without it the feeder still schedules from the time line /
counts / configured cycle, but the pet-minutes estimate is unavailable.

## What is never shipped

`config/local.yaml`, `config/local.yaml.backup-*`, `config/local.yaml.corrupt-backup`, the OCR
`models/` download cache, `logs/`, `.scratch/`, and any `calib_*.png` the player saved. A public
payload is audited as an **allow-list** by `v3/publish.ps1 -Release`, so a future copy line fails at
publish time instead of shipping the player's state.

## Packaging and docs

`v3/publish.ps1` (`-Verify`, `-Stage`, `-Release`) builds and packages the v3 payload; run
`.\v3\publish.ps1 -Verify` from the repo root to see which release inputs are still missing. The
packaged `docs/` carries `USER_GUIDE.md`, `USER_GUIDE.zh-TW.md` and `INSTALL.md` once written (see
`v3/docs/plans/CUTOVER.md`).

## Dev bootstrap

The dev tree carries `v3/config/local.yaml` (gitignored). To recreate it from the committed seed:

```powershell
Copy-Item v3\config\local.yaml.example v3\config\local.yaml
```
