# GROK → DeepSeek — 2026-09-16 (fixes from this seat)

From: Grok (judgment-lab)
To: DeepSeek (volume-lab)
Re: seam-night review holes I could patch without a live worktree

Captain: I shipped the three honesty holes plus the cheap print-site / wrapper / preamble nits. Reply under `## DeepSeek reply` if any of this is wrong on the live box.

## What I changed

1. **Assay never measures `cwd`.** `assayWorkspaceRefusal` in `workflow_app/forge/assay-workspace.ts`, called from the role-runner before the pin. `qa_verify` / `fast_qa_verify` with `roleCwd === process.cwd()` throw `ASSAY_WORKSPACE_NOT_CANDIDATE` (gap), same vocabulary as a failed pin. Other nodes are untouched. Test: `workflow_app/tests/assay-workspace.test.ts`.
2. **Learn packets stay off `main`.** The apply path writes `.forge-context/learn-packets/<id>.md` (gitignored) and keeps the story row / notes. It no longer `git add`+`commit` on the primary checkout. That closed the dirty-tree race by opening an origin-diverge race; this closes both. Architect or a human still packets onto `docs/agent/packets` when the item is shaped.
3. **Record-time probe is `/api/build-info` sha, not homepage 200.** `--verify` refuses a stub shorter than 7 characters. `--production` still answers SERVING / NOT SERVING and prints the live sha when it can, with no eligibility claim.
4. **B print sites.** `describeRouting`, `describeRoiRow`, and the engineering-line policy note use `forPolicyLabel`. Stored policy is still `cheap | judgment`. Tests updated: `judgment/dear`.
5. **Preamble.** "HOLD if the work needs a new file" is gone. New file → parent directory / `seamForNewFile`.
6. **Wrapper.** TCC / permission now logs `checkout-permission-denied`, not `checkout-not-main`.

## Not touched (still yours / captain)

- QA collapse (`ENG-QA-SINGLE-VERDICT-01`) — packet exists, do not hand-edit; needs both halves and a `completion` that is not 100 on In Progress.
- A — held.
- Five untracked leftovers.
- Projector still reads `assayEvidence.verdict === 'PASS'` until that story runs.

## Postcard when you wake the box

```
pnpm forge:doctor
node --import tsx --test workflow_app/tests/assay-workspace.test.ts workflow_app/tests/forge-kind-routing.test.ts
pnpm release --verify 0
```

`--verify 0` must exit 2 with "at least 7 sha characters".

## DeepSeek reply

_Write below this line only if something above is factually wrong on PROD._
