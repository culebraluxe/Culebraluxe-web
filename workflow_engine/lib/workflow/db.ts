// ---------------------------------------------------------------------------
// The workflow engine's database client — now the application's shared pool.
//
// This file used to build its OWN Neon client from the generic DATABASE_URL,
// which made the engine the last place in the system that decided its own
// connection and its own environment. It now takes the shared interactive handle
// (lib/neon-interactive), which is deliberately `pg.Pool`-backed:
//
//   * one pool per process — the same one the app and every operator script use;
//   * `sql.begin(cb)` is a REAL interactive transaction on ONE pooled client
//     (BEGIN / COMMIT / ROLLBACK), which is exactly the atomicity contract
//     engine.ts documents for every public operation;
//   * the target comes from the ONE declaration (lib/execution-target), never
//     from a generic url that could point anywhere.
//
// Its relative imports had drifted: `../../db/workflow-trace` did not resolve
// inside this standalone app, so they now point at the real modules explicitly.
//
// The fail-fast is preserved deliberately: this file already threw at module load
// when the database was unconfigured, and a loud startup failure beats an engine
// that quietly runs somewhere nobody named.
// ---------------------------------------------------------------------------
import { interactiveSql } from '../../../lib/neon-interactive';
import { describeControlPlane } from '../../../lib/execution-target';
import { WorkflowEngine } from './engine';
import { recordTraceEvent } from '../../../db/workflow-trace';
import type { RecordTraceInput } from '../../../db/workflow-trace';

const declared = describeControlPlane();
if (!declared.target) {
  throw new Error(
    declared.reason ?? 'workflow engine: the database environment is not declared',
  );
}

export const engine = new WorkflowEngine(interactiveSql, {
  // Observer-only flight recorder: workflow lifecycle events (WORKFLOW_STARTED /
  // NODE_ENTERED / TRANSITION_TAKEN / WORKFLOW_COMPLETED / WORKFLOW_FAILED) are
  // written through the SAME step transaction, are replay-safe, and NEVER gate
  // the engine (recordTraceEvent is contained and non-throwing).
  traceRecorder: (input, execute) =>
    recordTraceEvent(input as RecordTraceInput, execute),
});
