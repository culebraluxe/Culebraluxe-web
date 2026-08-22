# Isolated Worker Worktree Execution (ENG-21)

Evidence-driven hardening: **every worker execution gets its own isolated Git
worktree/branch rooted at an approved integration base**, so autonomous Forge
workers and interactive architecture agents never mutate the same checkout and
never touch ownership-unknown dirty files in another checkout.

Canonical model:

```
approved integration base ──▶ story worktree/branch ──▶ worker-local changes
        (pinned commit)          agent/<story>/<run>        (own checkout)
                                                              │
                                      local commit/evidence ──┘
                                      stop (never merge/push)
```

Boring Git worktree semantics only — no custom VCS machinery, no automatic
merge, rebase, push, or production promotion. Integration is an explicit
human-controlled checkpoint (ENG-22).

## What this changes

- The **primary checkout is never a worker scratch directory.** The scheduled
  worker (`pnpm agent:work`) and the interactive harness driver
  (`pnpm agent:runtime:deepseek`) execute each claimed story inside its own
  worktree at `../Culebraluxe-worktrees/<story>-<run>` on branch
  `agent/<story>/<run>`.
- Two concurrent workers can operate without modifying the same working tree —
  the database still enforces one active queue claim, but the workspace
  mechanism is lane-ready: each work item gets a deterministic, unique
  branch + worktree.
- Unrelated dirty files in another checkout are never touched: provisioning
  only READs the primary checkout (no stash / reset / clean / checkout-over),
  and cleanup refuses any workspace that still has uncommitted work.

## Naming (deterministic)

| artifact | form | example |
|---|---|---|
| branch | `agent/<story>/<run>` | `agent/eng-21/9f3c2b1a-…` |
| worktree dir | `<worktreesRoot>/<story>-<run>` | `../Culebraluxe-worktrees/eng-21-9f3c2b1a-…` |
| run id (worker) | the durable work-item id (uuid) | deterministic per work item |
| run id (CLI) | `--run <id>`, else `run-<stamp>-<rand>` | `run-lx3k2a-a1b2c3` |

Segments are sanitized (lowercase alphanumeric + hyphen, capped). The branch
and path are unique per run; provisioning refuses a duplicate branch/path
explicitly (never overwrites).

## Approved integration base

The base is an **explicit ref** (branch/tag/commit) — never "whatever HEAD
happens to be current" — and is pinned to a fixed commit at creation time:

- `AGENT_WORKSPACE_BASE_REF` — operator override (branch, tag, or commit hash).
- Default: the repository's canonical integration branch `main`.
- An unresolvable ref fails closed with an actionable error before any harness
  work begins; the claimed work item is terminalized as `Error` with the
  reason (no fake run is ever created).

## Commands

| command | purpose |
|---|---|
| `pnpm agent:workspace create <story> [--base <ref>] [--worker <id>] [--run <id>]` | provision one isolated branch + worktree (interactive agents: `cd` into the printed path and run there) |
| `pnpm agent:workspace status [<story>]` | list worker workspaces (branch / path / head) |
| `pnpm agent:workspace remove <story> [--run <id>]` | **safe explicit cleanup** — refuses uncommitted work, removes only the provisioner's own shared symlinks, always preserves the branch + commits |
| `pnpm agent:workspace help` | usage |

The scheduled worker and the interactive driver provision automatically
(default-on). Explicit environment:

| env | default | meaning |
|---|---|---|
| `AGENT_WORKSPACE_BASE_REF` | `main` | approved integration base ref |
| `AGENT_WORKSPACE_WORKTREES_ROOT` | `../Culebraluxe-worktrees` (next to the repo) | where worker worktrees live (must be OUTSIDE the primary checkout) |
| `AGENT_WORKSPACE_DISABLED` | unset | `1` restores the legacy shared-checkout path explicitly (escape hatch) |

Set these in the untracked `.env.scheduler` (sourced by
`scripts/agent-worker-once.sh`) or the environment.

## Shared resources

The provisioner symlinks `node_modules` and `.env.local` from the primary
checkout into each worktree (relative symlinks, never copies) so a worker does
not reinstall dependencies or re-create local config. Cleanup removes ONLY
symlinks it verifies point back at the primary checkout; the targets are never
deleted. Workers are instructed never to modify shared-linked configuration.

## Evidence

Every run persists (in `storyboard_story_run`):

- `commit_hash` — the worker's own HEAD in its isolated worktree. **Honest by
  contract:** when the checkout is still exactly at the approved base commit
  (the worker created no commit), `null` is persisted — a base commit is never
  recorded as if the worker had committed.
- a machine-scannable narrative line identifying the execution context:
  `Execution workspace: branch=<branch> worktree=<path> base=<ref>@<commit>`.

## Cleanup policy

- Cleanup is **explicit** (`pnpm agent:workspace remove`) — never automatic
  after a run, so evidence and the branch remain inspectable.
- Cleanup is **safe**: it refuses when the workspace has any uncommitted
  tracked/untracked change ("an abandoned workspace is cheaper than lost
  code"), never force-deletes, and always preserves the branch (all commits).
  If `git worktree remove` refuses (e.g. generated `.next/` remains), the
  error names the remaining files; remove them manually and retry.
- A crashed run leaves its worktree/branch in place for inspection; the next
  attempt uses a fresh work item → fresh branch/worktree.

## No auto-merge / push

Nothing in `lib/worker-workspace`, `scripts/workspace-cli.ts`, or the invoker
merges, rebases, pushes, or promotes. Worker branches are local-only;
integration into `main` is a deliberate human checkpoint (ENG-22).
