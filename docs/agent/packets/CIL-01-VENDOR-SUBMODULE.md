# CIL-01 — vendor the council repo as a pinned submodule

## Why

We want their protocol files on a SHA we control. Floating `main` would change
persona contracts under live verdicts. A copy-paste fork would rot.

## Units

- **A** — `git submodule add` at `vendor/council-of-high-intelligence`, checkout
  a tag (start `v1.2.0` unless a newer tag is current), commit `.gitmodules`
  with `update = none`.
- **B** — `vendor/council-pin.json` records repo, tag, sha, adapterVersion.
  SHA is `REPLACE_WITH_SUBMODULE_HEAD` in the pack; replace it in this story.
- **C** — `scripts/council-vendor-sync.sh` with `status|fetch|propose|pin|verify`.
- **D** — README note in `vendor/README.md`. Do not run `install.sh`.
- **E** — Vercel / CI clone fetches the submodule (`git submodule update --init`).

## Acceptance

- Fresh clone + `git submodule update --init` yields `SKILL.md` and `agents/`.
- `scripts/council-vendor-sync.sh verify` exits 0 on the pinned SHA.
- Changing checkout without rewriting the pin makes `verify` fail.
- No Claude/Codex skill files appear under `.claude/` from this story.

## Not this story

- No Neon, no service, no Forge XML, no portal UI.
- Not a lane.
