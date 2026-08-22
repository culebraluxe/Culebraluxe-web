// ---------------------------------------------------------------------------
// ENG-21 — Isolated Worker Worktree Execution: operator CLI.
//
//   pnpm agent:workspace create <story-id> [--base <ref>] [--worker <id>] [--run <id>]
//   pnpm agent:workspace status [<story-id>]
//   pnpm agent:workspace remove <story-id> [--run <id>]
//   pnpm agent:workspace help
//
// Delegates 1:1 to the provisioner (lib/worker-workspace). NO auto merge /
// rebase / push exists here; `remove` is conservative and always preserves the
// branch + commits. Interactive architecture agents use `create` to get an
// isolated checkout, then execute their harness inside it.
// ---------------------------------------------------------------------------

import {
  provisionWorkerWorkspace,
  listWorkerWorkspaces,
  removeWorkerWorkspace,
  resolveApprovedBaseRef,
} from '../lib/worker-workspace'
import { pathToFileURL } from 'node:url'

export function usage(): string {
  return [
    'Usage: pnpm agent:workspace <subcommand>',
    '',
    '  create <story-id> [--base <ref>] [--worker <id>] [--run <id>]',
    '                        create an isolated worker branch + worktree',
    '                        from an EXPLICIT approved base ref',
    '                        (default base: AGENT_WORKSPACE_BASE_REF or main)',
    '  status [<story-id>]   list isolated worker workspaces',
    '  remove <story-id> [--run <id>]',
    '                        safe cleanup (refuses uncommitted work; branch preserved)',
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
    case 'help':
    case undefined:
      return { code: sub === 'help' ? 0 : 2, text: usage() }
    default:
      return { code: 2, text: `unknown subcommand: ${sub}\n\n${usage()}` }
  }
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
