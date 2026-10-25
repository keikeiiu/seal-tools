# v3 config — the dev/install config tree

The core takes a **root** whose `config/` subdirectory holds its files:

```
seal-core.exe --root <repo>/v3
```

(The root is the directory *containing* `config/`, which is why the dev launch uses `<repo>/v3`, not
`<repo>/v3/config`.)

## What is here, and where it came from

| file | committed | provenance |
|---|---|---|
| `defaults.yaml` | yes | copied from `v2/config/defaults.yaml` — the committed source defaults (no calibration, no personal state). v3 adds its own keys here as they land. |
| `attributes.yaml`, `attributes.us.yaml` | yes | copied from `v2/config/`; the dictionaries `game.variants` names. |
| `local.yaml` | **no — gitignored** | created by the dev bootstrap from `v2/config/local.yaml.example`; the core is its only writer, and it is machine state. |

No personal data was copied: `v2/config/local.yaml` and the calibration PNGs stay where they are.

## Why a v3 tree instead of pointing the core at `v2/config`

v2's `local.yaml` is the player's live state — a v3 core pointed at it could write to it, and a shell
experiment must never risk that. This tree is the install's config directory in miniature; it starts
from the committed defaults so a fresh checkout can boot, and it diverges as v3's own settings land
(v2 keeps running untouched until the cut-over).

## Bootstrapping a fresh local.yaml

```powershell
Copy-Item v2\config\local.yaml.example v3\config\local.yaml
```

The core requires all three layers to exist (`defaults.yaml`, `local.yaml`, and the dictionary the
active variant names); a missing one is a boot refusal with the reason on stderr, by design.
