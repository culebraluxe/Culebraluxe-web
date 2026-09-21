// ---------------------------------------------------------------------------
// THE INTERRUPTED-SEQUENCE HARNESS — one place a crash between two durable
// writes is injected and the durable state left behind is read back.
//
// A crash proof used to re-implement its own injection (a bespoke `afterX`
// closure per test), so the crash POINT was an implementation detail of one
// file rather than a name the test states. Here a sequence is a list of named
// durable writes; `crashAfter(name)` runs up to and including that write and
// throws; `resume()` runs the rest. Every test therefore names the write it
// interrupts, and the exactly-once property is asserted by resuming twice.
//
// It is deliberately env-free: the store lives in memory, so the frozen proof
// runs with no connection and writes no database row.
// ---------------------------------------------------------------------------

import assert from 'node:assert/strict'

/** One durable write in a story-run sequence. It names itself. */
export type DurableWrite = {
  name: string
  run: () => void | Promise<void>
}

/** Thrown at the injected crash point, naming the write that committed last. */
export class InterruptedSequenceCrash extends Error {
  readonly writeName: string

  constructor(writeName: string) {
    super(`injected crash after durable write "${writeName}"`)
    this.name = 'InterruptedSequenceCrash'
    this.writeName = writeName
  }
}

export type InterruptedRun = {
  /** Run the writes in order and throw after the named one. */
  crashAfter: (writeName: string) => Promise<never>
  /** Finish the remaining writes; a second call after completion is a no-op. */
  resume: () => Promise<void>
  /** Names of the writes that have committed so far. */
  committed: () => string[]
  /** Names of the writes still to run. */
  pending: () => string[]
}

export function interruptedSequence(writes: DurableWrite[]): InterruptedRun {
  assert.ok(writes.length > 0, 'an interrupted sequence needs at least one durable write')
  const seen = new Set<string>()
  for (const write of writes) {
    assert.ok(write.name.trim().length > 0, 'every durable write must name itself')
    assert.ok(!seen.has(write.name), `duplicate durable write name "${write.name}"`)
    seen.add(write.name)
  }

  let cursor = 0
  return {
    async crashAfter(writeName) {
      const index = writes.findIndex((write) => write.name === writeName)
      assert.ok(index >= 0, `no durable write named "${writeName}" to crash after`)
      while (cursor <= index) {
        await writes[cursor].run()
        cursor += 1
      }
      throw new InterruptedSequenceCrash(writeName)
    },
    async resume() {
      while (cursor < writes.length) {
        await writes[cursor].run()
        cursor += 1
      }
    },
    committed: () => writes.slice(0, cursor).map((write) => write.name),
    pending: () => writes.slice(cursor).map((write) => write.name),
  }
}

// ---------------------------------------------------------------------------
// DRIVER VALUE-FORMAT GUARD.
//
// Postgres/Neon emits a `timestamptz::text` with the offset attached and a
// space separator (`2026-09-18 00:07:10.803394+00`), and a numeric as a decimal
// STRING. A hand-written fixture that normalised either into a shape the driver
// never produces is the defect class that hid the Z-append bug in
// ENG-FORGE-CLAIM-CLOCK-01, so a fixture value is asserted against the driver's
// own shape before a reader is allowed to trust it.
// ---------------------------------------------------------------------------

export const DRIVER_VALUE_FORMAT = {
  /** postgres `timestamptz::text`: space separator, fractional seconds, `+00` offset — never a trailing `Z`. */
  timestamptz: /^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}\.\d{2,6}\+00$/,
  /** postgres `numeric`/`bigint`/`count(*)` `::text`: a decimal string, never a JS number. */
  numericText: /^-?\d+$/,
} as const

export function isDriverTimestamptzText(value: unknown): boolean {
  return typeof value === 'string' && DRIVER_VALUE_FORMAT.timestamptz.test(value)
}

export function isDriverNumericText(value: unknown): boolean {
  return typeof value === 'string' && DRIVER_VALUE_FORMAT.numericText.test(value)
}

/**
 * Fail when a fixture value is a shape the driver never emits. The `Z` suffix
 * check is explicit (not just the regex) so the exact defect class is named.
 */
export function assertDriverValueShape(value: unknown, kind: 'timestamptz' | 'numeric'): void {
  if (kind === 'timestamptz') {
    assert.ok(
      !/[Zz]$/.test(String(value)),
      `a Z suffix is not a form the driver emits: ${JSON.stringify(value)}`,
    )
    assert.ok(
      isDriverTimestamptzText(value),
      `not a driver timestamptz::text form: ${JSON.stringify(value)}`,
    )
    return
  }
  assert.equal(
    typeof value,
    'string',
    `the driver emits numerics as strings, never ${typeof value}: ${JSON.stringify(value)}`,
  )
  assert.ok(isDriverNumericText(value), `not a driver numeric::text form: ${JSON.stringify(value)}`)
}
