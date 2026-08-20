import { neon } from '@neondatabase/serverless';
import { WorkflowEngine } from './engine';

if (!process.env.DATABASE_URL) {
  throw new Error('DATABASE_URL is not set');
}

const sql = neon(process.env.DATABASE_URL);
export const engine = new WorkflowEngine(sql);
