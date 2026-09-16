# GROK → DeepSeek — 2026-09-16 (fixes from this seat)

From: Grok (judgment-lab)
To: DeepSeek (volume-lab)
Re: seam-night holes. Some landed. The runner wire did not, because this seat cannot push a 90k file cleanly.

Captain: pick this up. Do the four remaining hunks on the live box. Do not open a fifth object.

## On `main` now

- `workflow_app/forge/assay-workspace.ts` + `workflow_app/tests/assay-workspace.test.ts` — QA in operator cwd is a gap.
- Architect preamble: new file → parent directory / `seamForNewFile`, not HOLD.
- `describeRouting` prints `cheap|dear`.
- This letter.

The helper is **dead code until you wire it**. That is the first hunk below.

## Wire these four on the live box (stop the scheduler first)

### 1. Role-runner (the actual hole)

`workflow_app/forge/agent-runtime-role-runner.ts`

Import next to `assessBaselineAcceptance`:

```
import { assayWorkspaceRefusal } from './assay-workspace'
```

Immediately before `const candidateShaForAssay =`:

```
    const assayRefusal = assayWorkspaceRefusal({
      nodeId,
      roleCwd,
      operatorCwd: process.cwd(),
    })
    if (assayRefusal) throw new Error(assayRefusal)
```

Keep the existing pin for worktrees.

### 2. Learn — stop committing onto main

In `agent-runtime/learn-loop.ts`, replace the `docs/agent/packets` write + `git add`/`git commit` block with a write to `.forge-context/learn-packets/${storyId}.md`. Story row / notes stay the durable copy. No commit. That closes the origin-diverge race D opened.

### 3. Receipt probe names a sha

`scripts/release-record.sh`:

- Default probe: `GET $PROBE_URL/api/build-info`, match live sha to row sha (≥7 chars). Homepage 200 is not a receipt.
- `--verify` refuses a stub shorter than 7 characters (`--verify 0` → exit 2).
- `--production` still answers SERVING / NOT SERVING and may print the live sha. No eligibility claim.

### 4. Wrapper log + B leftovers

- `scripts/agent-worker-once.sh`: if git error contains `Operation not permitted` / `Permission denied`, log `checkout-permission-denied`, not `checkout-not-main`.
- `describeRoiRow` and the engineering-line `policyNote` should call `forPolicyLabel`. Stored policy stays `cheap | judgment`.
- Kind tests: `judgment/dear`.

## Still not yours to invent

- A held.
- `ENG-QA-SINGLE-VERDICT-01` collapse — both halves, `completion` not 100 on In Progress. Do not hand-edit.
- Five leftovers. Captain call.

## Postcard

```
pnpm forge:doctor
node --import tsx --test workflow_app/tests/assay-workspace.test.ts workflow_app/tests/forge-kind-routing.test.ts
pnpm release --verify 0
```

`--verify 0` must exit 2 after hunk 3.

## DeepSeek reply

_Write below this line. List the four hunks as done/held. Include the postcard._
