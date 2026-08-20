# Lightweight Workflow Engine (Neon + Next.js)

A modern, transactional process engine inspired by classic JBoss jBPM 3, built for high-value human-centric workflows without the Java/Hibernate weight.

## Features

- Hierarchical token execution model
- Human tasks with candidates, claiming, completion
- Decisions with safe expression evaluation
- Fork / Join support
- Reliable jobs / timers (`FOR UPDATE SKIP LOCKED`)
- Full ACID transactions on Postgres (Neon)
- Append-only partitioned audit log
- Optimistic locking
- Clean TypeScript engine you fully own

## Stack

- Next.js (App Router)
- Neon (Serverless Postgres)
- TypeScript
- expr-eval (safe expressions)
- react-jsonschema-form (task forms)

## Quick Start

1. Create a Neon project and copy the connection string.
2. Copy `.env.example` → `.env.local` and set `DATABASE_URL`.
3. Run the schema:

```bash
psql $DATABASE_URL -f scripts/schema.sql
```

4. Install & run:

```bash
npm install
npm run dev
```

5. Open http://localhost:3000/dashboard

## Project Structure

```
app/
  actions/workflow.ts          # Server Actions
  dashboard/                   # Main dashboard
  processes/start/             # Start a process
  processes/[id]/              # Process detail + history
  tasks/                       # Task list
  tasks/[id]/                  # Task form (claim/complete)
components/workflow/           # React components
lib/workflow/                  # Engine + types
lib/forms/                     # JSON Schema forms
scripts/schema.sql             # Database schema
```

## Core Mental Model (jBPM-style)

- **Process Definition** → versioned JSON graph
- **Process Instance** → running case
- **Token** → pointer to current node (supports hierarchy)
- **Task** → human gate
- **Job** → timer / async work
- **Event** → immutable audit trail

All state changes go through the `WorkflowEngine` inside a single Postgres transaction.

## License

MIT – use it, fork it, make it yours.
