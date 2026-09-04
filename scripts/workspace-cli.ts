// ---------------------------------------------------------------------------
// ENG-21 — Isolated Worker Worktree Execution: operator CLI.
//
//   pnpm agent:workspace create <story-id> [--base <ref>] [--worker <id>] [--run <id>]
//   pnpm agent:workspace status [<story-id>]
//   pnpm agent:workspace remove <story-id> [--run <id>]
//   pnpm agent:workspace doctor [--story <id>]
//   pnpm agent:workspace help
//
// Delegates 1:1 to the provisioner (lib/worker-workspace). NO auto merge /
// rebase / push exists here; `remove` is conservative and always preserves the
// branch + commits. `doctor` is read-only: wrapper identity, tracking-ref
// state, orphan worktrees vs active items, per-profile readiness.
// ---------------------------------------------------------------------------

import {
  provisionWorkerWorkspace,
  listWorkerWorkspaces,
  removeWorkerWorkspace,
  resolveApprovedBaseRef,
} from '../lib/worker-workspace'
import { createAgentRuntimeRegistry } from '../agent-runtime/factory'
import { execFile } from 'node:child_process'
import { promisify } from 'node:util'
import { pathToFileURL } from 'node:url'

const execFileAsync = promisify(execFile)

export function usage(): string {
  return [
    'Usage: pnpm agent:workspace <subcommand>',
    '',
    '  create <story-id> [--base <ref>] [--worker <id>] [--run <id>]',
    '                        create an isolated worker branch + worktree',
    '                        from an EXPLICIT approved base ref',
    '                        (default base: AGENT_WORKSPACE_BASE_REF or origin/main)',
    '  status [<story-id>]   list isolated worker workspaces',
    '  remove <story-id> [--run <id>]',
    '                        safe cleanup (refuses uncommitted work; branch preserved)',
    '  doctor [--story <id>] read-only health: wrapper identity, origin/main,',
    '                        orphan worktrees, per-profile readiness (no mutations)',
    '  help                  this usage text',
    '',
    'The primary checkout is never a worker scratch directory. Workspaces live',
    'in ../Culebraluxe-worktrees next to the repository (override with',
    'AGENT_WORKSPACE_WORKTREES_ROOT).',
    '',
  ].join('\n')
}

function flag(args: string[], name: string): string | undefined {
  const i = args.indexOf(name)
  return i >= 0 ? args[i + 1] : undefined
}

export type WorkspaceCliDeps = {
  provision: typeof provisionWorkerWorkspace
  list: typeof listWorkerWorkspaces
  remove: typeof removeWorkerWorkspace
  /** ENG-FORGE-V6-VIS doctor deps (injected for deterministic tests). */
  doctor?: (opts: { storyId?: string }) => Promise<string>
}

export type WorkspaceCliOutcome = { code: number; text: string }

/** Dispatch one invocation. Each subcommand maps 1:1 to a provisioner function. */
export async function runWorkspaceCliCore(
  deps: WorkspaceCliDeps,
  argv: string[],
): Promise<WorkspaceCliOutcome> {
  const [sub, ...rest] = argv

  switch (sub) {
    case 'create': {
      const storyId = rest[0]
      if (!storyId) {
        return {
          code: 2,
          text: 'usage: pnpm agent:workspace create <story-id> [--base <ref>] [--worker <id>] [--run <id>]',
        }
      }
      const baseRef = flag(rest, '--base') ?? resolveApprovedBaseRef()
      const workerId = flag(rest, '--worker') ?? process.env.AGENT_WORKER_ID ?? 'local'
      const runId = flag(rest, '--run')
      const ws = await deps.provision({ storyId, workerId, baseRef, runId })
      return {
        code: 0,
        text: [
          `created isolated workspace for story ${ws.storyId}`,
          `  branch:       ${ws.branchName}`,
          `  worktree:     ${ws.worktreePath}`,
          `  base ref:     ${ws.baseRef} -> ${ws.baseCommit}`,
          `  shared links: ${ws.sharedLinks.join(', ') || '(none)'}`,
          '',
          `cd ${ws.worktreePath}   # one story, one isolated execution context`,
        ].join('\n'),
      }
    }
    case 'status': {
      const items = await deps.list()
      const filter = rest[0]
      const shown = filter ? items.filter((i) => i.storyId === filter) : items
      if (shown.length === 0) {
        return {
          code: 0,
          text: `no agent workspaces${filter ? ` for story ${filter}` : ''}`,
        }
      }
      const rows = shown
        .map(
          (i) =>
            `  ${i.branchName}\n    path: ${i.worktreePath}\n    head: ${i.head}`,
        )
        .join('\n')
      return { code: 0, text: `agent workspaces:\n${rows}` }
    }
    case 'remove': {
      const storyId = rest[0]
      if (!storyId) {
        return {
          code: 2,
          text: 'usage: pnpm agent:workspace remove <story-id> [--run <id>]',
        }
      }
      const runId = flag(rest, '--run')
      const result = await deps.remove({ storyId, runId })
      return {
        code: 0,
        text: [
          `removed worktree ${result.removedPath}`,
          `branch ${result.preservedBranch} preserved (all commits safe)`,
        ].join('\n'),
      }
    }
    case 'doctor': {
      // ENG-FORGE-V6-VIS — read-only health. Zero mutations: no pull/fetch,
      // no branch switch, no prune, no push. Reports facts; never repairs.
      const storyId = flag(rest, '--story')
      if (deps.doctor) {
        return { code: 0, text: await deps.doctor({ storyId }) }
      }
      return { code: 0, text: await runWorkspaceDoctor({ storyId }) }
    }
    case 'help':
    case undefined:
      return { code: sub === 'help' ? 0 : 2, text: usage() }
    default:
      return { code: 2, text: `unknown subcommand: ${sub}\n\n${usage()}` }
  }
}

/**
 * ENG-FORGE-V6-VIS — read-only workspace health.
 *
 * Zero mutations: no pull/fetch, no branch switch, no prune, no push.
 * Reports: approved base ref resolution, origin/main tracking state, orphan
 * worktrees (registered but path missing / unregistered path present), and
 * per-profile registry readiness. Never repairs; repair stays explicit via
 * `remove` / operator action.
 */
export async function runWorkspaceDoctor(opts: { storyId?: string } = {}): Promise<string> {
  const lines: string[] = ['forge workspace doctor (read-only, no mutations)']
  const repoRoot = process.cwd()

  async function git(args: string[]): Promise<{ ok: boolean; out: string }> {
    try {
      const { stdout } = await execFileAsync('git', args, { cwd: repoRoot, encoding: 'utf8' })
      return { ok: true, out: stdout.trim() }
    } catch (err) {
      const detail = ((err as { stderr?: string }).stderr ?? (err as Error).message ?? '').trim()
      return { ok: false, out: detail }
    }
  }

  // 1. Approved base ref + tracking-ref state.
  const baseRef = resolveApprovedBaseRef()
  const baseTip = await git(['rev-parse', '--verify', `${baseRef}^{commit}`])
  lines.push(
    baseTip.ok
      ? `approved base: ${baseRef} -> ${baseTip.out.slice(0, 12)}`
      : `approved base: ${baseRef} UNRESOLVABLE (${baseTip.out || 'no commit'}) — successor provisioning fails closed`,
  )
  const remoteHead = await git(['ls-remote', 'origin', 'refs/heads/main'])
  const remoteHash = remoteHead.ok ? remoteHead.out.split(/\s+/)[0] ?? '' : ''
  const tracking = await git(['rev-parse', '--verify', 'refs/remotes/origin/main'])
  lines.push(
    remoteHash
      ? `origin/main: remote=${remoteHash.slice(0, 12)} tracking=${tracking.ok ? tracking.out.slice(0, 12) : '(missing — publish syncs it)'}`
      : 'origin/main: remote unreadable (offline or no remote)',
  )
  const localMain = await git(['rev-parse', '--verify', 'refs/heads/main'])
  if (localMain.ok && tracking.ok && localMain.out !== tracking.out) {
    lines.push(
      `stale local main: refs/heads/main=${localMain.out.slice(0, 12)} != tracking=${tracking.out.slice(0, 12)} (successors use tracking ref, never local main)`,
    )
  }

  // 2. Worktrees vs filter.
  try {
    const items = await listWorkerWorkspaces({ repoRoot })
    const shown = opts.storyId ? items.filter((i) => i.storyId === opts.storyId) : items
    lines.push(`worktrees: ${shown.length} registered${opts.storyId ? ` for ${opts.storyId}` : ''}`)
    for (const item of shown.slice(0, 20)) {
      lines.push(`  ${item.branchName} head=${item.head.slice(0, 12)} path=${item.worktreePath}`)
    }
    if (shown.length > 20) lines.push(`  ... and ${shown.length - 20} more`)
  } catch (err) {
    lines.push(`worktrees: unreadable (${(err as Error).message})`)
  }

  // 3. Per-profile registry readiness (memoized shared registry; no execution).
  try {
    const registry = createAgentRuntimeRegistry()
    for (const profile of registry.listProfiles()) {
      const readiness = registry.inspectProfileReadiness(profile)
      lines.push(
        `profile ${profile}: ${readiness.ready ? 'ready' : `NOT READY — ${readiness.reason}`}`,
      )
    }
  } catch (err) {
    lines.push(`registry: unreadable (${(err as Error).message})`)
  }

  return lines.join('\n')
}

async function main(): Promise<void> {
  try {
    const outcome = await runWorkspaceCliCore(
      {
        provision: provisionWorkerWorkspace,
        list: listWorkerWorkspaces,
        remove: removeWorkerWorkspace,
      },
      process.argv.slice(2),
    )
    process.stdout.write(outcome.text + '\n')
    process.exitCode = outcome.code
  } catch (err) {
    // Fail clearly without a stack: provisioning/cleanup decisions are
    // deterministic and actionable (never force-delete, never silent default).
    const message = (err as Error)?.message ?? String(err)
    process.stderr.write(`agent:workspace: ${message}\n`)
    process.exitCode = 1
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  void main()
}
