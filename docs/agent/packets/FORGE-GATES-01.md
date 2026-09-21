# FORGE-GATES-01 — turn the harness rules we already state into gates that can fail

## Goal

Add the two gates that do not exist yet — a packet/harness **lint** (`pnpm forge:packet-lint`) and a
**silent-failure hunter** for Assay — and close the other three items of the intake as already-true with
tests, so no working code is thickened and no sixth lane, skill, or memory product is added.

## Why

Two intakes (the ECC catalog, the "5-layer memory" PDF) were reviewed on 2026-09-15. Both are real ideas
in fake packaging, and the review's own conclusion is the scope of this packet: **lift the write policy,
leave the catalog.** Forge already has the lanes; what it lacks is the two cheap parts that make the
existing rules enforceable — a scan of the *harness* (packets, skills, allowlists) and a check that
catches a swallowed failure in a diff.

The cost of not having them was paid tonight: a `catch` that silently dropped every error record, and a
`"use server"` file exporting an array, between them hid a dead action graph for hours (see
`docs/agent/MEMORY.md`, 2026-09-15). One lint and one pattern hunt would each have ended it in a round.

## Verified premises (checked against HEAD, 2026-09-15)

| Work order says | Reality |
|---|---|
| `agent-runtime/skills.ts` has five skills `neon, forms, workflow, ui, planner` | **TRUE** — `const KNOWN = ['neon','forms','workflow','ui','planner']` (line 4), and unknown tokens are already dropped (line 16) |
| `agent-runtime/write-policy.ts` revokes non-builder commits | **TRUE** — `revokeForbiddenCommit` drops the hash and requests a rewind |
| Packet format lists skills | **TRUE** — `docs/agent/packets/README.md` requires `## Skills`, "packs from `docs/agent/skills/`" |
| `agent-runtime/repo-context.ts` injects `MEMORY.md` / `CURRENT.md` into a prompt | **FALSE** — that file builds a ripwire/scout query packet; it injects no memory |
| The inject site exists elsewhere in-repo | **FALSE** — nothing in `agent-runtime/**` or `legacy/workflow_app/forge/**` reads `MEMORY.md`, `CURRENT.md` or `docs/agent/packets/*.md` into a prompt; the harness reads them (per `docs/agent/VENDOR-ADAPTERS.md`), not our code |

Two pre-existing inconsistencies found while checking (reported, not smuggled into scope):

1. `docs/agent/skills/` holds **nine** packs — `forms`, `neon`, `planner` (in `KNOWN`) plus
   `cruiser`, `knip`, `ripwire`, `rtk`, `semgrep`, `serena` (**not** in `KNOWN`), while `workflow` and
   `ui` are in `KNOWN` with **no file on disk**. The README's sentence is therefore wrong on disk.
2. Only `docs/agent/packets/ENG-FORGE-V6-SPEND.md` mentions skills at all, so the `## Skills` field is
   in the contract but unused in practice — the lint will find little at first, which is fine.


## Units

- **A — `scripts/forge-packet-lint.ts` + test.** Scans ONLY: `docs/agent/packets/*.md`,
  `docs/agent/skills/*.md`, `docs/agent/MEMORY.md`, `AGENTS.md`, and the allowlists in
  `agent-runtime/*.ts`. Fails (exit 1) on: a `## Skills` entry not in `KNOWN`; more than three packs in
  one packet; a `MEMORY.md` entry with no `YYYY-MM-DD` prefix; secret-shaped tokens (`sk-`, `ghp_`,
  `AKIA`, `postgres://…@`, a literal `.env.local` value); and any instruction telling Scout, Assay or
  Inspector to `git commit` (an `AGENTS.md` Never). It does **not** walk the monorepo, and it reports the
  skills/KNOWN mismatch from the table above as its own finding rather than failing on unrelated files.
- **B — `agent-runtime/silent-failure-patterns.ts` + test.** Takes an explicit **file list** (the changed
  files) and returns hits: empty `catch {}` / `catch (e) {}`; `.catch(() => [])` / `.catch(() => null)`;
  `console.error` in `app/` or `services/` with no nearby `captureServerError` / `captureError` /
  `withApiHandler` / `captureServerLog`; a `try` whose `catch` returns a bare 500 body. A NEW hit in the
  diff fails the change; pre-existing hits are reported only. This is `AGENTS.md`'s Error Capture
  Obligation as a gate rather than a sentence.
- **C — `docs/agent/MEMORY.md` header contract (10 lines, no rewrite of the log).**
  `status: observed | promoted | expired | contradicted`; only Inspector or the captain flips
  `observed → promoted`; a new fact that contradicts a promoted fact opens a ticket instead of
  overwriting; `CURRENT.md` is session residue, never policy. This is the one durable idea from the
  memory paper, and it lives where memory already lives.
- **D — thin tests for what already holds.** (i) `agent-runtime/skills.ts`: unknown tokens dropped, an
  empty `## Skills` injects zero text, five packs allowed, a sixth ignored. (ii) `write-policy.ts`:
  inspector/assay lanes still cannot commit, smith still can. Both are assertions about behaviour that
  exists — the work order's own clause: "already true, tests added."

## Explicitly closed as already-true (no code)

- **Fresh-context inspector.** `lane-policy.ts` already says it: smith "Do not review your own diff";
  inspector "Second opinion… Read-only… Do not silently patch"; assay "Read-only… Inventiveness is a
  defect." `write-policy.ts` already revokes the commit. The only addition is two lines of prompt text
  naming the silent-failure hunt targets — folded into Unit B rather than a new file.
- **Standing-context budget.** Nothing loads a catalog in-repo (`KNOWN` gating is the mechanism, and it
  works). There is no inject site to cap, so no fixture can honestly assert one. Recorded as a harness
  convention in `packets/README.md` if the captain wants it, not as code.

## Files to touch

- `docs/agent/packets/FORGE-GATES-01.md` (this file)
- `docs/agent/packets/README.md` (one line: the `## Skills` field is capped at three packs and must name
  `KNOWN` ids — and the skills-directory mismatch above)
- `docs/agent/MEMORY.md` (header only)
- `scripts/forge-packet-lint.ts`, `scripts/forge-packet-lint.test.ts`
- `agent-runtime/silent-failure-patterns.ts`, `agent-runtime/silent-failure-patterns.test.ts`
- `agent-runtime/skills.test.ts`, `agent-runtime/write-policy.test.ts` (extend)
- `package.json` — `forge:packet-lint` only

## Do not touch

`workflow_engine/**`; `docs/FORGE-V2.md` / `FORGE-V3.md`; portal, buyers, WhatsApp, CRM; any new skill
markdown; any schema.

## Assay (SCOPED)

```
pnpm forge:clean
pnpm forge:packet-lint
pnpm exec tsx --test scripts/forge-packet-lint.test.ts agent-runtime/silent-failure-patterns.test.ts \
  agent-runtime/skills.test.ts agent-runtime/write-policy.test.ts
git diff --check
```

## Acceptance criteria

1. A fixture packet naming a skill outside `KNOWN`, or listing four packs, or telling Inspector to
   commit — lint exits 1 with the reason.
2. A fixture diff adding `catch {}` in `app/` fails Assay; the same pattern pre-existing and outside the
   diff is reported, not blocking.
3. `MEMORY.md` keeps every existing entry and gains only the header block.
4. `skills.ts` still drops unknown tokens (asserted), and no prompt catalog is added anywhere.
5. `pnpm forge:packet-lint` is wired and named in this packet's Assay list.
6. Report in the three-item format from `AGENTS.md`.

## Out of scope (stop if you start these)

ECC agents or hooks; Mem0, a graph, or any memory product; self-writing memory; a repo-wide security
scan; a sixth lane; language-specific reviewer files; cross-harness adapters.

## Sign-off

**Signed off by the captain, 2026-09-15: "1 and 2 I feel like I just lived IRL... Yah I vote YES DO."**
That authorises Units **A and B** — the packet lint and the silent-failure hunter. Units **C** (MEMORY
status header) and **D** (extra tests) were left open that evening; the skills/KNOWN drift stays a
reported warning and the four V4 packets' `## Skills` debt is recorded in
`docs/agent/harness-lint-baseline.json` rather than silently rewritten.

What shipped: `scripts/forge-packet-lint.ts` + test (6 rules, `pnpm forge:packet-lint`, 0 failures with
the baseline recorded), `agent-runtime/silent-failure-patterns.ts` + test (4 patterns, new-hits-only),
`KNOWN_SKILLS` exported from `agent-runtime/skills.ts`, and `test:agent-runtime` wired (30 test files
that nothing had ever run — 223 tests, of which 5 pre-existing V9 topology tests fail and are reported,
not fixed here).

