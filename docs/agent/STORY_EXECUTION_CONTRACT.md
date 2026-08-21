# CulebraLuxe Story Execution Contract

Applies to **every** coding story. This is the shared interface between the
human owner, the architect/review model, and the coding agent. Read it before
starting any story; do not repeat it inside every `architect_brief`.

## Authoritative state

- The persistent Story Board is the authoritative CulebraLuxe backlog.
- Read the requested story before coding.
- Do not invent, rename, merge, or silently rewrite backlog stories.

## Story specification

Before coding, read the story's:

- `goal`
- `dependencies`
- `preconditions`
- `architect_brief`
- `context_refs`
- `acceptance_criteria`
- `postconditions`

If a field is empty, do not invent hidden requirements merely to fill it.

## Scope

- Work only the selected story.
- Do not expand scope because adjacent improvements look attractive.
- If a prerequisite genuinely blocks execution, record `Blocked` rather than
  silently creating a different project.

## Architecture

- Inspect the existing implementation before creating new abstractions.
- Reuse established seams and helpers where appropriate.
- `workflow_engine` must remain generic and domain-neutral.
- `workflow_app` contains the application integration.
- Application data remains the canonical business truth.
- The XML node ID is the workflow-state identity.
- `deal.stage` changes only through explicit application commands.
- Do not replace real engine/application behavior with an in-test fake merely
  to prove a story.

## Database

- The DEV database may be freely read, written, migrated, seeded, reset,
  queried, and repaired as needed.
- The production database must never be modified during autonomous execution.

## Execution state

When beginning work:

- start a `storyboard_story_run`
- set the story status to `In Progress`
- preserve the first `actual_start_at`
- snapshot the story execution specification

## Test / repair loop

- Implement within story scope.
- Run relevant tests and checks.
- Diagnose failures; repair them within scope; rerun tests.
- Use the real DEV database when integration behavior matters.
- Do not rely solely on mocks/fakes when the acceptance criteria concern real
  persistence or concurrency semantics.

## Completion

Mark `Complete` only when:

- acceptance criteria pass
- required postconditions hold
- relevant regression tests pass

Otherwise finish honestly as `Partial`, `Blocked`, `Failed`, `Deferred`, or
`Hold`.

## Result recording

On completion of a run, record:

- result status
- completion percentage
- concise execution notes
- commit hash when applicable
- tests/checks summary

Do not overwrite human story notes with execution output.

## Git

- Commit successful repository changes.
- Do not push unless explicitly instructed.
