import { spawnSync } from 'node:child_process'

/** Rust RE_supermodel host. cargo-per-call is refused. */
export function rustReWorkflow(args: string[]): string {
  const bin = process.env.RE_WORKFLOW_BIN?.trim()
  if (!bin) {
    throw new Error(
      'RE_WORKFLOW_BIN is not set. Build re-workflow and export the path. cargo run per call is refused.',
    )
  }
  const r = spawnSync(bin, args, { encoding: 'utf8', env: process.env })
  if (r.status !== 0) {
    throw new Error((r.stderr || r.stdout || `re-workflow ${args.join(' ')} failed`).trim())
  }
  return (r.stdout || '').trim()
}

export function parseStart(out: string): { instanceId: string; started: boolean } {
  const instance = /instance=(\S+)/.exec(out)?.[1]
  const started = /started=(true|false)/.exec(out)?.[1] === 'true'
  if (!instance) throw new Error(`re-workflow start produced no instance id: ${out}`)
  return { instanceId: instance, started }
}

export function parseReclaimed(out: string): number {
  const n = /reclaimed=(\d+)/.exec(out)?.[1]
  return n ? Number(n) : 0
}
