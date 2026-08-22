# Autonomous Failure / Escalation Policy (ENG-20)

Durable runtime/worker policy for unattended SDLC execution. Captured verbatim
from the ENG-20 story brief and wired at the minimum level required by the
smoke path (`scripts/agent-runtime-deepseek.ts` escalation branch + the shared
`AgentRuntimeAdapter` fail/cancel terminalization).

## Goal

Unattended workers should make reasonable progress, not pursue open-ended
recovery.

## Rules

1. Attempt the assigned task normally.

2. If blocked, perform bounded diagnosis:
   - inspect relevant logs/state;
   - reproduce once if practical;
   - identify likely root cause;
   - try only low-risk, reversible fixes within story scope.

3. Do not broaden scope to "fix the world".

4. Do not redesign architecture, replace major dependencies, change unrelated
   systems, or perform production-impacting actions merely to unblock yourself.

5. Escalate instead of continuing when:
   - the same failure repeats after reasonable retry;
   - root cause remains ambiguous after bounded investigation;
   - required access/credential/provider interaction is missing;
   - the fix materially expands story scope;
   - architecture judgment is required beyond captured guidance;
   - data integrity/security/production/destructive risk appears;
   - execution is hung or makes no meaningful progress for the configured
     timeout.

6. On escalation:
   - stop active work safely;
   - preserve git/DB/runtime state;
   - do not falsely mark Complete;
   - persist concise evidence:
       what failed · what was tried · likely root cause ·
       exact blocker · recommended human action;
   - terminalize Blocked/Error/HumanRequired as appropriate;
   - release the worker slot when safe;
   - allow the dispatcher to continue with the next eligible unattended story.

7. Prefer clean escalation to speculative heroics.

## Principle

A failed story should cost one story, not the rest of the night.

## Wiring in the codebase (ENG-20 minimum)

- The shared `AgentRuntimeAdapter.execute` terminalizes `failed` → work item
  `Error` + run `Failed`, and `cancelled` → work item `Cancelled` + run
  `Cancelled` (never falsely `Complete`).
- The DeepSeek driver wraps the invoker in an escalation branch: when the
  claim was made but execution cannot proceed, the work item is marked `Error`
  with concise escalation evidence (what failed / what was tried / likely root
  cause / recommended human action) and the worker slot is released.
- The fail-fast execution-target guard (`lib/execution-target.ts`) escalates
  BEFORE work begins whenever a non-PROD command would resolve to the
  production application/domain database.

No additional automation is built around this policy in ENG-20.
