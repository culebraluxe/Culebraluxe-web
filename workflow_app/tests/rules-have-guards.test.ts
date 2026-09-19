import assert from 'node:assert/strict'
import { existsSync, readFileSync } from 'node:fs'
import test from 'node:test'

// ---------------------------------------------------------------------------
// EVERY RULE HAS A GUARD, OR SAYS OUT LOUD THAT IT DOES NOT (2026-09-19).
//
// The failure this exists to stop: a rule is written in AGENTS.md, an agent's priors beat the prose, and nothing
// fails — QA-git crept back into a QA-named module with CI green, and worktrees were ripped out twice while the
// code that BUILDS them stayed shipped. Prose is not enforcement. So every bullet in the Never list must name
// its guard, and a bullet that has none must say so in the file, in the open, with a reason.
//
// The frozen count is the teeth: adding a rule with no guard fails here until somebody either writes the guard
// or writes the reason. Neither is silence.
// ---------------------------------------------------------------------------

const REPO_ROOT = new URL('../../', import.meta.url)
const AGENTS = readFileSync(new URL('AGENTS.md', REPO_ROOT), 'utf8')

/** The bullets of the Never section, as written: from `Never` (line 40) to the next heading (`## Project`). */
function neverBullets(): string[] {
  const lines = AGENTS.split('\n')
  const start = lines.findIndex((line) => line.trim() === 'Never')
  if (start < 0) return []
  const rest = lines.slice(start + 1)
  const end = rest.findIndex((line) => /^#{1,3} /.test(line))
  const section = (end < 0 ? rest : rest.slice(0, end)).join('\n')
  return section
    .split(/\n(?=- )/)
    .map((block) => block.trim())
    .filter((block) => block.startsWith('- '))
}

/** Guard files with no automated check today. A number, frozen on purpose: it may fall, never rise silently. */
const UNGUARDED = 2

test('every Never rule names a guard, or states in the open that it has none', () => {
  const bullets = neverBullets()
  assert.ok(bullets.length >= 8, `the Never section must still be read: found ${bullets.length} bullets`)

  for (const bullet of bullets) {
    const guard = /guard: ([^\n]+)/.exec(bullet)?.[1]?.trim()
    assert.ok(
      guard,
      `a Never rule has no guard line: "${bullet.slice(0, 90)}…". Add \`guard: <test path>\` naming the check ` +
        'that fails when this rule is broken, or `guard: NONE — <reason>` if it truly has none. Silence is how ' +
        'the QA-git and worktree creeps happened.',
    )
    if (guard.startsWith('NONE')) {
      assert.ok(guard.length > 'NONE — '.length, `an unguarded rule must give a reason: "${guard}"`)
      continue
    }
    const path = new URL(guard, REPO_ROOT)
    assert.ok(existsSync(path), `${guard} is named as the guard for a Never rule but does not exist`)
    assert.match(
      readFileSync(path, 'utf8'),
      /test\(/,
      `${guard} is named as a guard but contains no test`,
    )
  }
})

test('the number of UNGUARDED rules is frozen — it may fall, it may not rise unnoticed', () => {
  const unguarded = neverBullets().filter((bullet) => /guard: NONE/.test(bullet))
  assert.ok(
    unguarded.length <= UNGUARDED,
    `${unguarded.length} Never rules have no guard, and this file froze the allowance at ${UNGUARDED}. ` +
      'Either write the check, or raise the number in this fence deliberately — with the reason next to the rule.',
  )
})
