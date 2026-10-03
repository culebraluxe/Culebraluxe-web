# Handoff — the lane handover, the machine, and what CI is actually reporting

Session of 2026-10-03, agent in `src/lane-deep`. Read with `AGENTS.md` rules 1–4 open.

## 1. STATUS — what is true right now

| # | Fact | How to check it |
| --- | --- | --- |
| S1 | `lane/deep` is this session's desk and is exactly `origin/main` | `git -C /Users/Shared/dev/src/lane-deep log --oneline -1` → `5f97a013`; `git diff main --name-only` → empty |
| S2 | The lane's pre-move replay is preserved, not discarded | `git rev-parse refs/archive/lane-deep-replay` → `692d99c7` (also still `lane/claude`'s tip) |
| S3 | `cargo` now finds the shared target dir with no shell export | `~/.cargo/config.toml` `[build] target-dir = /Users/Shared/dev/build/rust`; a throwaway crate in `/tmp` compiled into that dir and left no local `target/` |
| S4 | Registered worktrees are main + the three lanes, and nothing else | `git worktree list` → 4 entries (`/private/tmp/ocwt`, `/private/tmp/pristine`, `~/Documents/Culebraluxe-web-roles`, `/private/tmp/m2` removed) |
| S5 | `~/Documents` holds no checkout of this repo | `ls ~/Documents \| grep -iE 'culebra\|forge-seam'` → personal PDFs/photos only |
| S6 | Four launchd jobs are *repointed on disk* but their loaded config is still the dead path | `grep -c Documents/Culebraluxe-web ~/Library/LaunchAgents/com.culebraluxe.*.plist` → 0; `launchctl list \| grep culebraluxe` → `agent-worker` 2, `apple-sync`/`apple-local-listener`/`calendar-sync` 1 |
| S7 | CI on `b386458a` is red in two independent places, and ten steps below them never ran | `gh run view 37106427840` (gates) and `37106427834` (cutover) |
| S8 | The deployed `apple-sync-launcher` is a signed arm64 Mach-O with the dead path baked in; the deployed copy is byte-identical to the in-tree release build again | `codesign -v apple-messages-export/.build/release/apple-sync-launcher` → exit 0; `cmp` against the deployed copy → clean |
| S9 | The one pre-existing rule-3 exposure found was in a deleted tree, and its copy is now owner-only | the archived `.env.local` from `Culebraluxe-web-claude` is `600` (it was `644` in an iCloud-synced folder) |

## 2. HOLDS — do not act on these

| # | Held | Who holds it | What an agent must do |
| --- | --- | --- | --- |
| H1 | The four `forge_seam__001..004` failures | the workflow-lane owner (`CURRENT.md` "Four tests fail") | Do not edit the seam expectations, and do not exclude them from the run to make CI green (`AGENTS.md` "you may not make it pass by narrowing it") |
| H2 | The osv-scanner advisory failing static gates | the dependency/triage owner | A row that is dated and owned; never silence the ledger (`AGENTS.md` T2 rule) |
| H3 | The four deferred module trees (`cli/src/apple_mail`, `cli/src/forge/lint`, `middle/model/src/applemail`, `middle/model/src/apple_messages`) | a live agent, by name in `gates.yml` | Never run `cargo fmt --all` over them |
| H4 | `com.culebraluxe.agent-worker` — the engine | the captain | Do not bootstrap it; starting it resumes production work and spends tokens |
| H5 | `whatsapp/whatsapp-webhook.zip` and `data/skills/svar-react`, deleted by `da16f5ea` | the captain | Unexplained, not assumed intended; recovery is `git show 97785410:<path> > <path>` |

## 3. WHERE TO LOOK — task → the one place

| Your task | Read | The files you touch |
| --- | --- | --- |
| Why lanes exist, how to land a slice | `docs/agent/LAYOUT.md` §Lanes, `AGENTS.md` rule 4 | your lane worktree, then `main` from the main checkout |
| The three-tier move and its two guards | `docs/agent/LAYOUT.md` §The layout move | `scripts/restructure-domain-layout.sh`, `scripts/validate-move-script.sh`, `tests/tests/arch_boundary__013__*` |
| What the machine is (engine, roles, jobs) | `docs/agent/CURRENT.md` §What runs | `forge/src/roles/`, `forge/definitions/FORGE_SDLC-v6.xml` |
| What CI runs, step by step | `.github/workflows/gates.yml` | — |
| Where the target dir and logs live | `AGENTS.md` §Where the tree lives, `docs/agent/LAYOUT.md` §The tree | `~/.cargo/config.toml` (machine-local — never commit it), `build/rust`, `build/logs` |

## 4. DONE — what landed, with the receipts

| Commit | What it changed | The gate that ran |
| --- | --- | --- |
| `5f97a013` on `origin/main` and `origin/lane/deep` | `scripts/tmp-go.sh` derives its repo root from its own location instead of `cd`-ing to `~/Documents/Culebraluxe-web`, which the 2026-10-01 move emptied — so the script exited 1 before reaching the engine it exists to launch | `bash -n scripts/tmp-go.sh` → OK. Shell-only: no `Cargo.toml`/`Cargo.lock` change and no Rust or `web/ui` path, so the pre-push hook's lock and wasm checks do not apply |

## 5. NOT VERIFIED — the honest gaps

- No compile and no test run this session: no `cargo check`, no `cargo nextest`, no `pnpm slice:check`. The one commit is a shell script.
- The CI run for `5f97a013` (started 08:00:48Z) was still in progress when this was written; it inherits both pre-existing failures.
- The repointed plists have not been reloaded, so nothing proves the new paths work end to end. The evidence for the *path* is the sibling job that was already correct: `com.culebraluxe.wip-snapshot` points at `/Users/Shared/dev/src/Culebraluxe-web` and exits 0.
- The launcher's baked-in path is *restored to the broken original*, not fixed; only a Swift rebuild fixes it.
- `~/.cargo/config.toml` was proven with a throwaway crate, not with a real workspace build. `build/` currently holds `rust/` (184 K), `logs/`, and the archive below.
- `build/archive-2026-10-03-from-documents` (84 K: two `.env*`, three `forge_arch_seam` drafts, `gsd-core`) is a new directory in a directory `LAYOUT.md` describes as `rust/` + `logs/` only.
- Disk: data volume 747 Gi used / 1.1 Ti free (`df -h /System/Volumes/Data`). This session freed ~1.3 G, below that resolution. The `/` reading of 13 Gi is the sealed system volume and means nothing.
- Nothing was reformatted, no deferred tree was touched, and the other two lanes were read but never written.

## 6. OPEN — the next actions, in order

1. Reload the three sync jobs, then hand `agent-worker` to the captain: `launchctl bootout gui/501/<label>` then `launchctl bootstrap gui/501 ~/Library/LaunchAgents/<label>.plist`. Finished when `launchctl list | grep culebraluxe` reports 0 and the job logs show a completed run.
2. Rebuild `apple-sync-launcher` from `apple-messages-export/` (`Package.swift`) with the new path and redeploy through `scripts/apple-sync-agent.mjs`. Finished when the deployed binary shows 0 occurrences of the dead path and `codesign -v` verifies.
3. CI, the structural half: give the steps *below* a known-red step the `if: ${{ !cancelled() }}` treatment the same file already uses for `install cargo-nextest` (`gates.yml:348`). Ten steps are skipped today — `rust browser compile`, `rust warnings are errors`, `rust file boundedness`, both dead-TS ledger steps, and the cutover proof that Forge runs without `legacy/`. Finished when `gh run view <id> --json jobs` shows them *running*; the job may still fail, which is the point of the change.
4. CI, the borrowed half: H1 and H2 belong to their owners. The receipt for `5f97a013` should name them rather than imply the cause is unknown.
5. Decide on `build/archive-2026-10-03-from-documents` (move or delete — the seam drafts there are superseded by `tests/tests/forge_arch_seam__*`).

## 7. ASK THE OWNER

- Push `refs/archive/lane-deep-replay`, or leave it local like `refs/wip/*`? Push → the three lanes' replay commits become pullable; leave → they stay local and `lane/claude` remains their only reference.
- Reload `agent-worker` now? Yes → production resumes claiming work; no → it stays failing on the old config until someone reloads it.
- Were the two deletions in `da16f5ea` intended? Yes → one line in `MEMORY.md` closes it; no → both restore from `97785410`.
