// ---------------------------------------------------------------------------
// ENG-FORGE-SYNC-GUARD-01 — the Forge lane cannot start anywhere but PROD.
//
// The regression this pins is concrete and happened: `pnpm forge:engine` launched
// with `process.env.EXECUTION_ENV ?? 'DEV'` and the role runner parsed the target
// with a 'DEV' fallback, so SILENCE meant DEV. The lane then recorded DEV on the
// work item and (with APP_ENV unset) resolved its control-plane database to DEV —
// which is how DEV kept accumulating Forge data while the board lived in PROD.
//
// These tests are pure: fake env objects, no database, no OpenCode, no engine.
// The integration-style cases call the real runner, which is safe precisely
// because the guard is the runner's FIRST statement — the refusal is what stops
// it before any claim, so nothing is touched.
// ---------------------------------------------------------------------------

import { describe, it } from 'node:test'
import assert from 'node:assert/strict'

import {
  assertForgeExecutionTarget,
  assertForgeLaneMayStart,
  ForgeEnvironmentError,
  resolveForgeExecutionTarget,
} from '../forge/forge-execution-target'
import {
  assertForgeExecutionTarget as reExportedGuard,
  ForgeEnvironmentError as ReExportedError,
} from '../forge/forge-board-sync'
import { createAgentRuntimeForgeRoleRunner } from '../forge/agent-runtime-role-runner'

/** Run `fn` with exactly these environment variables, restoring afterwards. */
function withEnv(vars: Record<string, string | undefined>, fn: () => void): void {
  const keys = ['EXECUTION_ENV', 'APP_ENV', 'VERCEL_ENV']
  const saved = keys.map((k) => [k, process.env[k]] as const)
  try {
    for (const k of keys) delete process.env[k]
    for (const [k, v] of Object.entries(vars)) {
      if (v === undefined) delete process.env[k]
      else process.env[k] = v
    }
    fn()
  } finally {
    for (const [k, v] of saved) {
      if (v === undefined) delete process.env[k]
      else process.env[k] = v
    }
  }
}

/** `withEnv` for async bodies: the env is held for the whole awaited callback. */
async function withEnvAsync(
  vars: Record<string, string | undefined>,
  fn: () => Promise<void>,
  alsoClear: string[] = [],
): Promise<void> {
  const keys = ['EXECUTION_ENV', 'APP_ENV', 'VERCEL_ENV', ...alsoClear]
  const saved = keys.map((k) => [k, process.env[k]] as const)
  try {
    for (const k of keys) delete process.env[k]
    for (const [k, v] of Object.entries(vars)) {
      if (v === undefined) delete process.env[k]
      else process.env[k] = v
    }
    await fn()
  } finally {
    for (const [k, v] of saved) {
      if (v === undefined) delete process.env[k]
      else process.env[k] = v
    }
  }
}

describe('ENG-FORGE-SYNC-GUARD-01 lane-start guard', () => {
  it('resolves PROD from EXECUTION_ENV, its alias, or APP_ENV alone', () => {
    assert.equal(resolveForgeExecutionTarget({ EXECUTION_ENV: 'PROD' }), 'PROD')
    assert.equal(resolveForgeExecutionTarget({ EXECUTION_ENV: 'production' }), 'PROD')
    assert.equal(resolveForgeExecutionTarget({ APP_ENV: 'production' }), 'PROD')
    // EXECUTION_ENV is the canonical knob and wins over APP_ENV.
    assert.equal(
      resolveForgeExecutionTarget({ EXECUTION_ENV: 'PROD', APP_ENV: 'development' }),
      'PROD',
    )
  })

  it('refuses a DEV target instead of coercing it', () => {
    assert.throws(() => resolveForgeExecutionTarget({ EXECUTION_ENV: 'DEV' }), ForgeEnvironmentError)
    assert.throws(
      () => resolveForgeExecutionTarget({ EXECUTION_ENV: 'development' }),
      ForgeEnvironmentError,
    )
    assert.throws(
      () => resolveForgeExecutionTarget({ APP_ENV: 'development' }),
      ForgeEnvironmentError,
    )
  })

  it('refuses SILENCE: no configured target is not an implicit DEV', () => {
    // This is the whole regression. lib/execution-target.ts maps an absent APP_ENV
    // to 'DEV' because that is right for application code; a Forge lane gets no
    // implicit target, so every way of saying nothing must throw.
    assert.throws(() => resolveForgeExecutionTarget({}), ForgeEnvironmentError)
    assert.throws(() => resolveForgeExecutionTarget({ PATH: '/usr/bin' }), ForgeEnvironmentError)
    assert.throws(() => assertForgeExecutionTarget(undefined), ForgeEnvironmentError)
    assert.throws(() => assertForgeExecutionTarget(null), ForgeEnvironmentError)
    assert.throws(() => assertForgeExecutionTarget('staging'), ForgeEnvironmentError)
  })

  it('lets a lane start only when BOTH the target and the control plane are PROD', () => {
    assert.equal(
      assertForgeLaneMayStart({ env: { EXECUTION_ENV: 'PROD' }, controlPlane: 'prod' }),
      'PROD',
    )
    // resolveDbTarget() returns the lowercase form; both spellings are accepted.
    assert.equal(
      assertForgeLaneMayStart({ env: { APP_ENV: 'production' }, controlPlane: 'PROD' }),
      'PROD',
    )
  })

  it('refuses a PROD lane whose control plane resolved to DEV (the DEV-data direction)', () => {
    assert.throws(
      () => assertForgeLaneMayStart({ env: { EXECUTION_ENV: 'PROD' }, controlPlane: 'dev' }),
      (error: unknown) => {
        assert.ok(error instanceof Error)
        assert.match(error.message, /control plane must be PROD/)
        // The message has to name the knob, or the operator cannot act on it.
        assert.match(error.message, /APP_ENV=production/)
        return true
      },
    )
  })

  it('refuses a DEV lane even when the control plane is PROD', () => {
    assert.throws(
      () => assertForgeLaneMayStart({ env: { EXECUTION_ENV: 'DEV' }, controlPlane: 'prod' }),
      (error: unknown) => {
        assert.ok(error instanceof Error)
        assert.match(error.message, /Forge runs against PROD only/)
        return true
      },
    )
  })

  it('keeps the guard available on its old import path (board sync re-export)', () => {
    // forge-board-sync.ts owned this guard; it now lives in
    // forge-execution-target.ts and is re-exported, so the sync script and its own
    // test file keep working unchanged.
    assert.equal(reExportedGuard, assertForgeExecutionTarget)
    assert.throws(() => reExportedGuard('DEV'), ReExportedError)
  })

  it('refuses a lane start from the real runner when the target is DEV, before any claim', async () => {
    const runner = createAgentRuntimeForgeRoleRunner({ workerId: 'guard-test' })
    await withEnvAsync({ EXECUTION_ENV: 'DEV', APP_ENV: 'production' }, async () => {
      // The runner is async, so the refusal lands as a rejected promise. That it
      // rejects at all — rather than reaching the database and failing there — is
      // the proof that no task was claimed and no OpenCode spawned.
      await assert.rejects(
        () => runner('smith', { taskId: 't', processInstanceId: 'p' } as never),
        ForgeEnvironmentError,
      )
    })
  })

  it('refuses a caller-declared DEV target even when the environment says PROD', async () => {
    const runner = createAgentRuntimeForgeRoleRunner({
      workerId: 'guard-test',
      executionEnvironment: 'DEV',
    })
    await withEnvAsync({ EXECUTION_ENV: 'PROD', APP_ENV: 'production' }, async () => {
      await assert.rejects(
        () => runner('smith', { taskId: 't', processInstanceId: 'p' } as never),
        ForgeEnvironmentError,
      )
    })
  })

  it('refuses a lane start when nothing is configured at all', async () => {
    const runner = createAgentRuntimeForgeRoleRunner({ workerId: 'guard-test' })
    await withEnvAsync({}, async () => {
      await assert.rejects(
        () => runner('smith', { taskId: 't', processInstanceId: 'p' } as never),
        ForgeEnvironmentError,
      )
    })
  })

  it('lets a PROD lane start past the guard, and never dials a database to prove it', async () => {
    // Box: "With target PROD, start proceeds." This story owns the guard, so the
    // assertion is that the guard does NOT stop a PROD lane — the run gets through
    // it to the next stage. The database URLs are cleared so that next stage is a
    // configuration failure rather than a live connection: a unit test must never
    // dial production, and a rejection that is not a ForgeEnvironmentError is
    // exactly what "the guard let it through" looks like.
    const runner = createAgentRuntimeForgeRoleRunner({ workerId: 'guard-test' })
    await withEnvAsync(
      { EXECUTION_ENV: 'PROD', APP_ENV: 'production' },
      async () => {
        await assert.rejects(
          () => runner('smith', { taskId: 't', processInstanceId: 'p' } as never),
          (error: unknown) => {
            assert.ok(
              !(error instanceof ForgeEnvironmentError),
              'the guard must not stop a lane whose target and control plane are PROD',
            )
            return true
          },
        )
      },
      ['DATABASE_URL_PROD', 'DATABASE_URL_DEV', 'DATABASE_URL'],
    )
  })
})
