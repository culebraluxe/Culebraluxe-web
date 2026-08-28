import { neon } from '@neondatabase/serverless';
import { WorkflowEngine } from './engine';
import { recordTraceEvent } from '../../db/workflow-trace';
import type { RecordTraceInput } from '../../db/workflow-trace';

if (!process.env.DATABASE_URL) {
  throw new Error('DATABASE_URL is not set');
}

const sql = neon(process.env.DATABASE_URL);
export const engine = new WorkflowEngine(sql, {
  // Observer-only flight recorder: workflow lifecycle events (WORKFLOW_STARTED /
  // NODE_ENTERED / TRANSITION_TAKEN / WORKFLOW_COMPLETED / WORKFLOW_FAILED) are
  // written through the SAME step transaction, are replay-safe, and NEVER gate
  // the engine (recordTraceEvent is contained and non-throwing).
  traceRecorder: (input, execute) =>
    recordTraceEvent(input as RecordTraceInput, execute),
});
