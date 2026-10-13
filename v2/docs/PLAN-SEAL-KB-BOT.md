# Plan — a Seal Online knowledge bot for a Telegram group

Written 2026-10-10. **DESIGN ONLY — nothing built, nothing configured, no channel added.**
This file is the package: the mechanism as measured, the shape, the one decision that
gates it, the Telegram-side steps, and a ready-to-copy `AGENTS.md`.

Not part of the suite. This is infrastructure around the repo, not a v2 tool.

---

## 1. What it is

A Telegram **group** bot that answers Seal Online questions — install, calibration, what
each tool does, why it behaves that way, pet feeding values, what changed in a release —
grounded in this repository's own documentation rather than in the model's memory.

A group rather than a DM because several people can ask in one place and read each
other's answers, and the history stays.

---

## 2. What already exists (measured, not assumed)

**The connector** is `~/.dsh/dsh-im-connect/`. `channels.json` holds per-channel config:

```
id, platform, name, assistant, cwd, permission, agentPreset,
commandPermissions, privateAccess, enabled, receiveEnabled, config
```

**Two channels are configured today, and each one is already a workspace-scoped bot:**

| channel | cwd |
|---|---|
| `telegram_f3e2ac6c9f0a` | `C:\Git_repo\seal-tools` |
| `telegram_6ea407b18813` | `C:\Git_repo\media-stack-docker-compose\ptcg trace` |

So "one bot per project" is **the existing pattern**, not a new idea. A knowledge bot is
another channel with a different `cwd` — nothing more.

**The persona slot is `AGENTS.md` in that `cwd`.** This is not inference: the session
that wrote this file loaded both `~/.dsh/AGENTS.md` (global) and
`C:\Git_repo\seal-tools\AGENTS.md` (workspace) automatically. That is where "what you
are, how you answer, what you must never do" lives.

**`assistant` is not the persona** — it is only the model
(`deepseek-official / deepseek-flash / reasoningEffort max`). `agentPreset: standard`.

**Groups are already supported and enabled:**

```json
"commandPermissions": { "dm":    { "enabled": true, "users": [] },
                        "group": { "enabled": true, "users": [] } }
```

`group.enabled` is already `true`; `group.users` is empty, which is the field that
decides *who* may command the bot in a group.

**Access today:** `privateAccess: "approved"`, `allowlist` = `5871938453` (one user id).

**The knowledge is already written** — no authoring work needed:

| source | size |
|---|---|
| `v2/docs/*.md` | 41 files, 949 KB |
| root `README.md` | release table + quick start |
| `v2/config/attributes.yaml` | the attribute dictionary (game wording) |
| `v2/docs/PET-DATA.md` | per-pet feeding values |

---

## 3. Why there is no RAG here

The corpus is under a megabyte and it is **live** — it changes with every commit. An
embedding index would need a rebuild step, would drift from the files, and would answer
from stale text. An agent that greps and reads the files per question is smaller, always
current, and cites what it used.

RAG becomes worth it at a corpus size where reading is too slow, or where the files are
not text. Neither is true here.

---

## 4. The shape

A new channel, e.g. `seal-kb`:

| field | value |
|---|---|
| `platform` | `telegram` |
| `cwd` | **the decision in §5** |
| `AGENTS.md` | the draft in §7, dropped into that `cwd` |
| `permission` | as narrow as the preset allows — this bot reads, it does not write |
| `agentPreset` | `standard` |
| `assistant` | the same model as the other channels |
| `commandPermissions.group` | `enabled: true`, `users` = whoever may ask |
| `allowlist` | the group's chat id (negative, `-100…`) |

---

## 5. The one decision: where `cwd` points ⚑

**DSH confines _writes_ to the workspace but not _reads_.** Verified this session: under
`workspace-write` I read `~/.dsh` freely — many times — while writes below the workspace
root were refused. So a bot whose `cwd` is this repo can be *asked* for anything in it,
and the repo holds real secrets:

| file | what it is |
|---|---|
| `v1/checkin/checkin_config.yaml` | **game account passwords, in plaintext** |
| `v1/checkin/sessions/*.json` | **live login tokens** |
| `v2/config/local.yaml` | personal calibration and state |
| `~/.dsh/.credentials.yaml` | API credentials |

An instruction in `AGENTS.md` is **not enforcement**. It is what a well-behaved agent
follows, not what a determined asker cannot get around.

So the choice is:

**Option A — the group is yourself only.** Then `cwd` can be
`C:\Git_repo\seal-tools` and the bot reads the live docs directly. Nothing goes stale,
and there is no audience to protect it from. This is the simplest correct answer.

**Option B — other people are in the group.** Then `cwd` must be a folder of its own
containing **documentation copies only**, and the bot must never be pointed at the repo:

```
C:\Git_repo\sealonline-kb\
    AGENTS.md          <- §7, with the "must never" section kept
    docs\              <- copies of v2/docs/*.md, README.md, PET-DATA.md
```

The cost of B is that copies go stale. Two ways to pay it: a copy step run after doc
changes, or a directory junction from the KB folder to `v2\docs` (which keeps it live but
re-introduces the reach — so only acceptable if the junction is the *only* thing exposed).

**Do not** run B by pointing at the repo and asking the prompt to behave. That is the
version that leaks the day someone asks the right question.

---

## 6. Telegram-side steps (yours, not the agent's)

1. **Add the bot to the group** — any group admin can; no DSH change needed for this step.
2. **Disable Group Privacy** in @BotFather: `/setprivacy` → pick the bot → **Disable**.
   Without this the bot receives only `/commands` and @mentions, and normal conversation
   never reaches DSH. This is the step people miss, and it looks like "the bot is dead".
3. **Get the group's chat id** — group ids are **negative** (`-100…`), unlike the user id
   currently in `allowlist`. It goes on the allowlist for the new channel.
4. **Decide `commandPermissions.group.users`** — empty means everyone in the group can
   command it. Fill it if that is not what you want.

---

## 7. Draft `AGENTS.md` (ready to copy)

```markdown
# Seal Online knowledge bot

You answer questions about **Seal Online** and the tools in this repository.

## What you are

A documentation assistant in a Telegram group. You are not a general assistant, and you
do not take on unrelated work.

## Where the knowledge is

The documentation lives in the seal-tools repository:

    C:\Git_repo\seal-tools\v2\docs\                  the specification
    C:\Git_repo\seal-tools\README.md                 release history, quick start
    C:\Git_repo\seal-tools\v2\config\attributes.yaml attribute names (game wording)

Read the files. Never answer a game question from memory — if you did not read it, you
do not know it.

## How to answer

- 繁體中文 by default; otherwise answer in the language you were asked in.
- **Cite the file** you used, e.g. `v2/docs/CALIBRATION.md`.
- Quote the sentence that decides it when the exact wording matters.
- **If the docs do not answer it, say so plainly.** Never invent a config key, a
  coordinate, a version number or a behaviour.
- Keep it short — a group is not a report reader.

## What you must never do

- **Never read or reveal** `v1/checkin/checkin_config.yaml` (account passwords),
  `v1/checkin/sessions/` (login tokens), `v2/config/local.yaml` (personal calibration),
  or anything under `~/.dsh`. If asked for them, refuse and say why.
- **Never modify any file.** If asked to change something, say who can.
- **Never run the game tools or the check-in from here.** This bot answers questions; it
  does not act.

## Useful starting points

| question | file |
|---|---|
| install, board, first calibration | `v2/docs/INSTALL.md` |
| every card, tab and button | `v2/docs/USER_GUIDE.md` (zh-TW twin alongside) |
| what each tool sends to the game | `v2/docs/TOOL-FLOWS.md` |
| how it works, and why | `v2/docs/DESIGN.md` |
| calibrating this machine | `v2/docs/CALIBRATION.md` |
| the pet feeder and feeding values | `v2/docs/PET-TAB-DESIGN.md`, `v2/docs/PET-DATA.md` |
| what changed in a release | `README.md` |
| why a coordinate or cursor behaves oddly | `v2/docs/COORDINATES.md` |
```

**If you chose Option B**, change the "Where the knowledge is" block to point at
`docs\` beside this file and delete the repository path, so the bot has no reason to look
outside its own folder.

---

## 8. Open questions

1. **Option A or B** — is anyone but you in the group? (§5)
2. **Who may command it** — `commandPermissions.group.users`, empty or named ids.
3. **Should it ever write?** e.g. log questions it could not answer to a file. Nice for
   improving the docs; it means a write permission this plan otherwise avoids.
4. **Language** — 繁體中文 assumed, since the audience and `USER_GUIDE.zh-TW.md` agree.

---

## 9. What was measured, and what was only reasoned about

**Measured:** the `channels.json` field list and the two existing channels; that
`assistant` holds only model settings; that `AGENTS.md` is loaded from the workspace
(demonstrated by the session that wrote this); `commandPermissions.group.enabled = true`
with empty `users`; `privateAccess: "approved"`; the allowlist contents; the size of
`v2/docs` (41 files / 949 KB); and the existence of the secret-bearing files listed in §5.

**Reasoned, not tested here:** the Telegram group mechanics — Group Privacy, negative
group ids, admin rights — which are standard Bot API behaviour but were not exercised.
The §7 draft has never been loaded by a bot.

**Not attempted:** no channel was created, no config file under `~/.dsh` was touched, and
no bot was added to any group.
