// ---------------------------------------------------------------------------
// Persistence test harness (ENG-04) — real PostgreSQL test support for the
// workflow engine hardening stories (CRM-14B, ENG-09, later CRM-14F/recovery).
//
// Runs against the DEV Neon branch via the shared interactive-transaction
// adapter (lib/neon-interactive). Every fixture is isolated behind a unique
// tenant_id, so tests can assert exact persisted state and clean up
// deterministically (child-first, by tenant). FakeSql/in-memory substitutes
// are NOT acceptable proof for locking/isolation/rollback/concurrency —
// this tier exists for exactly those semantics.
//
// This file imports application infrastructure (lib/neon-interactive) only to
// reach the DEV database; the ENGINE product code itself stays generic.
// ---------------------------------------------------------------------------
import { randomUUID } from 'node:crypto'

import { interactiveSql } from '../../../lib/neon-interactive'
import { WorkflowEngine } from '../../lib/workflow/engine'
import type { ProcessGraph } from '../../lib/workflow/types'
import { stubEvaluator, makeApp } from '../fixtures'

export type { ProcessGraph }

/** Fail loudly if the engine schema is not present in the DEV database. */
export async function assertEngineSchema(): Promise<void> {
  const rows = await interactiveSql`select to_regclass('process_definitions') as pd`
  const pd = rows[0]?.pd as string | null
  if (!pd) {
    throw new Error(
      'workflow_engine schema is missing from the DEV database. Apply ' +
        'workflow_engine/scripts/schema.sql + scripts/migrations/001,002 to DATABASE_URL_DEV first.',
    )
  }
}

/** Ensure the tiny probe table used by isolation/overlap demos exists. */
export async function ensureProbeTable(): Promise<void> {
  await interactiveSql`
    create table if not exists tunit_probe (
      id bigserial primary key,
      tenant_id uuid not null,
      note text not null
    )
  `
}

/** Ensure the app-side command-effect table used by the ENG-09 boundary proof. */
export async function ensureCommandEffectTable(): Promise<void> {
  // Test-only table. Tenant-scoped so concurrent persistence files (the
  // test:persistence glob may run files in parallel processes) cannot
  // interfere with each other's rows.
  await interactiveSql`drop table if exists tunit_command_effect`
  await interactiveSql`
    create table tunit_command_effect (
      command_id text primary key,
      tenant_id uuid not null,
      effect_count integer not null default 1
    )
  `
}

export type FixtureEngineOptions = {
  app?: any
  now?: () => Date
  hooks?: any
}

/**
 * One isolated persistence fixture: a unique tenant, a seeded definition, an
 * engine bound to the real DEV database, and persisted-state inspection +
 * deterministic cleanup.
 */
export class PersistenceFixture {
  readonly tenantId = randomUUID()

  async seedDefinition(key: string, version: number, graph: ProcessGraph): Promise<string> {
    const rows = await interactiveSql`
      insert into process_definitions (
        tenant_id, key, version, name, description, definition, status, created_by
      ) values (
        ${this.tenantId}, ${key}, ${version}, ${key}, null,
        ${JSON.stringify(graph)}::jsonb, 'active', 'persistence-test'
      )
      returning id
    `
    return rows[0].id as string
  }

  makeEngine(options: FixtureEngineOptions = {}): WorkflowEngine {
    return new WorkflowEngine(interactiveSql as any, {
      evaluate: stubEvaluator,
      app: options.app ? makeApp(options.app) : makeApp(),
      now: options.now ?? (() => new Date()),
      hooks: options.hooks,
    })
  }

  async rows<T = Record<string, any>>(
    strings: TemplateStringsArray,
    ...values: any[]
  ): Promise<T[]> {
    return (interactiveSql as any)(strings, ...values) as Promise<T[]>
  }

  async instance(id: string): Promise<Record<string, any> | null> {
    const rows = await interactiveSql`
      select * from process_instances where id = ${id} limit 1
    `
    return rows[0] ?? null
  }

  async tokens(instanceId: string) {
    return interactiveSql`
      select * from tokens where process_instance_id = ${instanceId} order by created_at, id
    `
  }

  async tasks(instanceId: string) {
    return interactiveSql`
      select * from tasks where process_instance_id = ${instanceId} order by created_at, id
    `
  }

  async jobs(instanceId: string) {
    return interactiveSql`
      select * from jobs where process_instance_id = ${instanceId} order by created_at, id
    `
  }

  async events(instanceId: string, type?: string) {
    if (type) {
      return interactiveSql`
        select * from process_events
        where process_instance_id = ${instanceId} and event_type = ${type}
        order by id, created_at
      `
    }
    return interactiveSql`
      select * from process_events where process_instance_id = ${instanceId} order by id, created_at
    `
  }

  async commands(instanceId: string) {
    return interactiveSql`
      select * from process_commands where process_instance_id = ${instanceId} order by id
    `
  }

  /** Deterministic teardown for this fixture's tenant. */
  async cleanup(): Promise<void> {
    // Helper tables may not exist if no test created them; guard each delete.
    try {
      await interactiveSql`delete from tunit_probe where tenant_id = ${this.tenantId}`
    } catch {
      /* table absent */
    }
    try {
      await interactiveSql`delete from tunit_command_effect where tenant_id = ${this.tenantId}`
    } catch {
      /* table absent */
    }
    // process_events has no FK to process_instances; delete by tenant first.
    await interactiveSql`delete from process_events where tenant_id = ${this.tenantId}`
    // process_instances cascades tokens/tasks/jobs/process_commands.
    await interactiveSql`delete from process_instances where tenant_id = ${this.tenantId}`
    await interactiveSql`delete from process_definitions where tenant_id = ${this.tenantId}`
  }
}
