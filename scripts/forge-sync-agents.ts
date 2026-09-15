// ---------------------------------------------------------------------------
// forge:sync-agents — regenerate the managed block in every vendor pointer file.
//
//   pnpm forge:sync-agents           # rewrite the block where it exists, no-op if identical
//   pnpm forge:sync-agents --check   # exit 1 when a block is missing or has drifted
//   pnpm forge:sync-agents --format json
//
// VENDOR-ADAPTERS.md says vendor files are one-line pointers and never a second source
// of truth. This is the tool that keeps them pointers: the content is generated from
// constants in lib/agent-vendor-block.ts, every guardrail is anchored to a sentence that
// must still exist in AGENTS.md, and we never create a vendor file that was not already
// there. `pnpm forge:packet-lint` fails when a block drifts, so handbook-first survives a
// hand edit.
// ---------------------------------------------------------------------------

import { execFileSync } from 'node:child_process'
import { existsSync, readFileSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'

import {
  MANAGED_VENDOR_FILES,
  hasVendorBlock,
  orphanedGuardrails,
  renderVendorBlock,
  upsertVendorBlock,
  vendorBlockDrifted,
} from '../lib/agent-vendor-block'

function repoRoot(): string {
  try {
    return execFileSync('git', ['rev-parse', '--show-toplevel'], { encoding: 'utf8' }).trim()
  } catch {
    return process.cwd()
  }
}

export type SyncStatus = 'wrote' | 'unchanged' | 'would-write' | 'drifted'

export type SyncResult = {
  files: Array<{ path: string; status: SyncStatus; hasBlock: boolean }>
  absent: string[]
  orphaned: string[]
}

/**
 * Pure enough to test: takes the file contents in, returns what it would do. The CLI
 * underneath is the only thing that touches the disk.
 */
export function planSync(input: {
  /** path -> current content, for files that exist. */
  present: ReadonlyMap<string, string>
  absent: readonly string[]
  agentsMd: string
}): SyncResult & { writes: Array<{ path: string; content: string }> } {
  const block = renderVendorBlock()
  const orphaned = orphanedGuardrails(input.agentsMd).map((guardrail) => guardrail.anchoredBy)
  const files: SyncResult['files'] = []
  const writes: Array<{ path: string; content: string }> = []

  for (const [path, current] of input.present) {
    const hasBlock = hasVendorBlock(current)
    if (!hasBlock) {
      files.push({ path, status: 'would-write', hasBlock: false })
      writes.push({ path, content: upsertVendorBlock(current, block) })
      continue
    }
    const next = upsertVendorBlock(current, block)
    if (next === current) {
      files.push({ path, status: 'unchanged', hasBlock: true })
      continue
    }
    files.push({ path, status: vendorBlockDrifted(current, block) ? 'drifted' : 'would-write', hasBlock: true })
    writes.push({ path, content: next })
  }

  return { files, absent: [...input.absent], orphaned, writes }
}

function main(): number {
  const argv = process.argv.slice(2)
  const check = argv.includes('--check')
  const json = argv.includes('--format') && argv[argv.indexOf('--format') + 1] === 'json'
  const root = repoRoot()

  const agentsPath = join(root, 'AGENTS.md')
  if (!existsSync(agentsPath)) {
    console.error('FAIL  AGENTS.md not found — the handbook is the source of this block')
    return 1
  }
  const agentsMd = readFileSync(agentsPath, 'utf8')

  const present = new Map<string, string>()
  const absent: string[] = []
  for (const path of MANAGED_VENDOR_FILES) {
    const full = join(root, path)
    if (existsSync(full)) present.set(path, readFileSync(full, 'utf8'))
    else absent.push(path)
  }

  const plan = planSync({ present, absent, agentsMd })

  if (plan.orphaned.length > 0) {
    // Refuse to write a rule the handbook no longer contains. This is the anchor check
    // doing the one job that makes replication safe.
    console.error('FAIL  guardrail(s) no longer anchored in AGENTS.md:')
    for (const anchor of plan.orphaned) console.error(`      "${anchor}"`)
    console.error('      fix the handbook or the guardrail constant before syncing.')
    return 1
  }

  if (!check) {
    for (const write of plan.writes) {
      writeFileSync(join(root, write.path), write.content, 'utf8')
    }
  }

  const drifted = plan.files.filter((file) => file.status === 'drifted')
  const unblocked = plan.files.filter((file) => !file.hasBlock)

  if (json) {
    console.log(
      JSON.stringify(
        {
          mode: check ? 'check' : 'write',
          files: plan.files.map((file) => ({ ...file, status: check ? (file.status === 'unchanged' ? 'unchanged' : 'drifted') : file.status })),
          absent: plan.absent,
        },
        null,
        2,
      ),
    )
  } else {
    for (const file of plan.files) {
      const status = check
        ? file.status === 'unchanged'
          ? 'ok      '
          : file.status === 'drifted'
            ? 'FAIL    '
            : 'MISSING '
        : file.status === 'unchanged'
          ? 'unchanged'
          : 'wrote   '
      console.log(`${status}${file.path}${file.hasBlock ? '' : ' (no managed block yet)'}`)
    }
    if (plan.files.length === 0) console.log('no vendor pointer files present')
    console.log(
      `\nforge:sync-agents — ${plan.files.length} vendor file(s), ${plan.files.length ? plan.files.length - plan.writes.length : 0} unchanged, ` +
        `${plan.writes.length} ${check ? 'would change' : 'written'}, ${plan.absent.length} not present`,
    )
    console.log(
      `  not present (we do not create vendor files): ${plan.absent.length ? plan.absent.join(', ') : 'none'}`,
    )
    if (check && (drifted.length > 0 || unblocked.length > 0)) {
      console.log('  run `pnpm forge:sync-agents` to regenerate.')
    }
  }

  if (check && (drifted.length > 0 || unblocked.length > 0)) return 1
  return 0
}

if (process.argv[1] && /(^|\/)forge-sync-agents\.ts$/.test(process.argv[1])) {
  process.exit(main())
}
