import { mkdtempSync } from 'node:fs'
import { homedir } from 'node:os'
import { join } from 'node:path'
import { readFileSync } from 'node:fs'
import { startOpenCodeRun } from '../agent-runtime/opencode/opencode-client'

// Probe 3 — V5-21 arithmetic-continuity test (the user's accumulator idea).
// Three flash turns in ONE project, resumed via `--continue` (no session-id
// derivation needed). A running total must accumulate 10 -> 20 -> 30. That is a
// hard, un-fakeable continuity signal: turn N can only answer correctly if the
// session actually carried the prior turns' state.
//   Run: node --import tsx scripts/oc-probe3.ts

const CLI = process.env.OPENCODE_BIN ?? `${homedir()}/.opencode/bin/opencode`
const FLASH = process.env.PROBE_FLASH ?? 'deepseek/deepseek-v4-flash'
const BUDGET_MS = Number(process.env.PROBE_BUDGET_MS ?? 150_000)
const LOG = `${homedir()}/.local/share/opencode/log/opencode.log`

function readLog(): string {
  try {
    return readFileSync(LOG, 'utf8')
  } catch {
    return ''
  }
}
function lastSessionIndex(text: string): number {
  const re = /session\.id=(ses_[A-Za-z0-9]+)/g
  let last = -1
  let m: RegExpExecArray | null
  while ((m = re.exec(text)) !== null) last = m.index
  return last
}
function sessionsAfter(text: string, minIndex: number): string[] {
  const out: string[] = []
  const re = /session\.id=(ses_[A-Za-z0-9]+)/g
  let m: RegExpExecArray | null
  while ((m = re.exec(text)) !== null) if (m.index > minIndex) out.push(m[1])
  return out
}
function firstInt(s: string): number | null {
  const m = /\d+/.exec(s)
  return m ? Number(m[0]) : null
}

async function runOnce(cwd: string, model: string, task: string, resume: boolean): Promise<{ sessionId: string | null; value: number | null; raw: string; exitCode: number | null }> {
  const beforeIdx = lastSessionIndex(readLog())
  const handle = startOpenCodeRun({ cliBin: CLI, cwd, model, task, autoApprove: true, continueSession: resume })
  const timer = setTimeout(() => { console.warn('   [budget exceeded — cancelling]'); handle.cancel() }, BUDGET_MS)
  const res = await handle.promise
  clearTimeout(timer)
  const after = sessionsAfter(readLog(), beforeIdx)
  const sessionId = after.length > 0 ? after[after.length - 1] : null
  const value = firstInt(res.stdout)
  console.log(`   [turn ${resume ? 'resume' : 'start'}] exit=${res.exitCode} value=${value} session=${sessionId ?? 'NONE'}`)
  return { sessionId, value, raw: res.stdout.trim(), exitCode: res.exitCode }
}

async function main(): Promise<void> {
  const real = mkdtempSync(join('/tmp', `forge-oc-probe3-${Date.now()}`))
  console.log(`[probe3] model=${FLASH} cwd=${real}`)
  const expected = [10, 20, 30]

  console.log('[probe3] TURN 1 (start session): current total is 0, add 10')
  const t1 = await runOnce(real, FLASH, 'We are keeping a running total. The current total is 0. Add 10 to it. Reply with ONLY the new total as a bare integer (no prose, no code fence).', false)
  if (!t1.sessionId) console.log('[probe3] note: could not read a session id from the opencode log')

  console.log('[probe3] TURN 2 (--continue same project): add 10 to the carried-over total')
  const t2 = await runOnce(real, FLASH, 'Continuing the SAME session: take the running total you computed earlier in this conversation, add 10 to it. Reply with ONLY the new total as a bare integer (no prose, no code fence).', true)

  console.log('[probe3] TURN 3 (--continue same project): add 10 again')
  const t3 = await runOnce(real, FLASH, 'Continuing the SAME session: take the running total from the previous turn in this conversation, add 10 to it. Reply with ONLY the new total as a bare integer (no prose, no code fence).', true)

  const ids = [t1.sessionId, t2.sessionId, t3.sessionId]
  const sameId = t1.sessionId && t2.sessionId === t1.sessionId && t3.sessionId === t1.sessionId
  const got = [t1.value, t2.value, t3.value]
  const accumulated = got[0] === 10 && got[1] === 20 && got[2] === 30
  console.log(`[probe3] expected totals 10,20,30 -> got ${JSON.stringify(got)}`)
  console.log(`[probe3] all turns share one session id: ${sameId} (${ids.join(', ')})`)
  console.log('[probe3] turn 2 raw:', JSON.stringify(t2.raw.slice(0, 120)))
  console.log('[probe3] turn 3 raw:', JSON.stringify(t3.raw.slice(0, 120)))
  const pass = accumulated && sameId
  console.log(`[probe3] RESULT: ${pass ? 'PASS — session carries state across --continue turns (10->20->30)' : 'HOLD/FAIL — state did not accumulate (session not continued)'}`)
}

main().then(() => process.exit(0), (e) => { console.error('[probe3] ERROR', (e as Error)?.message ?? String(e)); process.exit(1) })
