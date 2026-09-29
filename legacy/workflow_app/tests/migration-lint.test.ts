import assert from 'node:assert/strict'
import { test } from 'node:test'
import { execFileSync } from 'node:child_process'
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { runStaticGate } from '@/legacy/workflow_app/forge/forge-static-gate'

// The fence drives the engine's migration gate with explicit fixture files. squawk must be on
// PATH for a refusal to be produced; when it is not (a minimal dev environment), the gate fails
// closed, so the fence SKIPS rather than asserting behaviour the environment cannot show — the
// same precedent as forge-static-gate.test.ts's dependency-cruiser skip.
function squawkAvailable(): boolean {
  try {
    execFileSync('squawk', ['--version'], { stdio: 'ignore', timeout: 20_000 })
    return true
  } catch {
    return false
  }
}

const SKIP = squawkAvailable() ? false : 'squawk is not installed in this checkout'

function withFixture(name: string, sql: string, fn: (file: string) => void): void {
  const dir = mkdtempSync(join(tmpdir(), 'migration-lint-'))
  const file = join(dir, name)
  writeFileSync(file, sql)
  try {
    fn(file)
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
}

test('unsafe migration is refused with the squawk rule id', { skip: SKIP }, () => {
  withFixture(
    'unsafe.sql',
    'CREATE INDEX idx_unsafe ON t (a);\nALTER TABLE t ADD CONSTRAINT c CHECK (a > 0);\n',
    (file) => {
      const result = runStaticGate({ workspace: process.cwd(), migrationFiles: [file] })
      assert.equal(result.migrationRan, true)
      assert.equal(result.migrationOk, false)
      assert.equal(result.ok, false)
      const all = result.migrationFindings.join('\n')
      assert.match(all, /require-concurrent-index-creation/)
      assert.match(all, /constraint-missing-not-valid/)
    },
  )
})

test('safe migration passes the gate', { skip: SKIP }, () => {
  withFixture(
    'safe.sql',
    "SET lock_timeout = '4s';\nSET statement_timeout = '30s';\n" +
      'CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_safe ON t (a);\n' +
      'BEGIN;\nALTER TABLE t ADD CONSTRAINT c CHECK (a > 0) NOT VALID;\nCOMMIT;\n',
    (file) => {
      const result = runStaticGate({ workspace: process.cwd(), migrationFiles: [file] })
      assert.equal(result.migrationRan, true)
      assert.equal(result.migrationOk, true)
      assert.deepEqual(result.migrationFindings, [])
    },
  )
})

test('no changed migrations lints nothing', { skip: SKIP }, () => {
  // No explicit files: the script selects the changed migration set itself. With no changed
  // migrations the gate reports NO findings — never the historical profile over db/migrations.
  const result = runStaticGate({ workspace: process.cwd() })
  assert.equal(result.migrationRan, true)
  assert.equal(result.migrationOk, true)
  assert.deepEqual(result.migrationFindings, [])
})
