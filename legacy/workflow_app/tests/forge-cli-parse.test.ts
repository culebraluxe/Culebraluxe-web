import assert from 'node:assert/strict'
import test from 'node:test'
import { execFileSync } from 'node:child_process'
import { readdirSync } from 'node:fs'

// ---------------------------------------------------------------------------
// THE FIELD CHANNEL MUST PARSE.
//
// Every decision a Forge role records travels through scripts/forge-handoff.mjs. If that
// file does not parse, the write fails, the reader finds no row, and the gate reports
// "No decision was recorded in fields" — which reads exactly like a model that ignored its
// instructions. Observed live on 2026-09-13: a duplicated `const pool` declaration was
// committed, a SPLIT dogfood HOLDed on it, and the only reason it was diagnosed at all is
// that the model pasted the SyntaxError into its own second attempt.
//
// This is the cheapest possible fence for that class: a CLI that cannot parse can never be
// the silent cause of a HOLD again. TypeScript entry points are covered by `tsc --noEmit`.
// ---------------------------------------------------------------------------

test('every operator CLI in scripts/ parses', () => {
  const clis = readdirSync('scripts')
    .filter((name) => name.endsWith('.mjs') && !name.endsWith('.test.mjs'))
    .map((name) => `scripts/${name}`)

  assert.ok(clis.length > 0, 'the guard must find CLIs to check, or it proves nothing')

  const broken: string[] = []
  for (const cli of clis) {
    try {
      execFileSync(process.execPath, ['--check', cli], { stdio: 'pipe' })
    } catch (error) {
      const out = `${(error as { stderr?: Buffer }).stderr?.toString() ?? ''}`.trim()
      broken.push(`${cli}: ${out.split('\n').slice(0, 3).join(' ')}`)
    }
  }
  assert.deepEqual(broken, [], `a CLI that does not parse cannot record anything: ${broken.join(' | ')}`)
})
