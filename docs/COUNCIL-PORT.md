# Council of High Intelligence — port into CulebraLuxe

Status: **[proposed]** 2026-09-19. Nothing in this folder is live on HEAD until the
story stack is executed against `culebraluxe/Culebraluxe-web`.

This is the operator + architect contract for vendoring
[0xNyk/council-of-high-intelligence](https://github.com/0xNyk/council-of-high-intelligence)
as a **git submodule**, wrapping it as a **domain service**, persisting verdicts
in **Neon**, and wiring it into Forge and the main app **without a seventh lane
and without a second workflow**.

## 0. Laws (do not violate)

Copied from living repo policy. A port that breaks these is a refuse, not a
clever adapter.

1. **One workflow, the rows.** Verdicts live in Neon. No worktree, no
   `.council-sessions/` directory, no file that outlives the command.
2. **One fact, one writer.** `services/council` is the only writer of
   `council_session` / `council_verdict`. Architect findings, Lead PRE lines,
   portal UI, and Assay checkpoints *call the service*. They do not INSERT.
3. **Envelope-only writes.** New business writes go through
   `CouncilService.execute(...)`. No raw SQL from routes, Forge lanes, or
   scripts except `db/council.ts` behind the repository.
4. **Paid code > git SHA.** The vendor SHA labels the protocol that produced a
   verdict. It never gates, voids, or replays a paid decision.
5. **No extra Forge lane.** Topology stays Scout → Architect → Lead → Smith →
   QA → DEV_OPS. Council is a *service call* from Architect / Lead PRE / Assay
   / app domains. `lanes.ts` does not gain `council`.
6. **Do not run upstream `install.sh` against this repo.** Their installer
   writes Claude/Codex skill files. That is a human sidecar, not the kernel.
7. **Juice > squeeze.** Default mode is `duo` (two seats) or `refuse`. Full
   18-member council is not a production operation.
8. **Validate at the boundary.** Provider JSON is unknown until Zod (or the
   existing runtime schema helper) accepts it. A TypeScript `as` cast is not
   validation.

## 1. What the submodule is — and is not

| Vendored (read-only protocol) | Owned by CulebraLuxe (the product) |
|---|---|
| `SKILL.md` + host mirrors | `services/council` |
| `agents/*.md` frontmatter (method, polarity, blind spots, word caps) | Neon tables + `db/council.ts` |
| polarity pairs, triad names, hemlock / depth / cutoff prose | Verdict schema v1 (`CouncilVerdict`) |
| MIT license, changelog, tags | Adapter version `COUNCIL_ADAPTER_VERSION` |
| | Pin file `vendor/council-pin.json` |
| | Domain panels (deal, match, intake, forge) |

Not vendored into runtime:

- `install.sh`
- `.claude-plugin/`
- their Python roster validator as a live gate
- their `/council` slash command as a Forge node
- historical persona names in the portal UI

The busts stay in `vendor/`. The app speaks `frame-break` and `rebuild`, or
domain seats `comp` / `buyer-fit` / `fragility`.

## 2. Target tree after port

Drop this pack onto repo root. Paths are relative to `Culebraluxe-web/`.

```
vendor/council-of-high-intelligence/     git submodule (pinned SHA)
vendor/council-pin.json                  owned pin + adapter version
scripts/council-vendor-sync.sh           update / verify / refuse-drift
services/council/
  index.ts
  types.ts
  council-service.ts
  repository.ts
  protocol.ts                            reads vendor SKILL + agents
  panels.ts                              CulebraLuxe seat maps
  refuse.ts                              when-not-to-convene
  model-port.ts                          LLM port (no vendor CLI)
db/council.ts                            repository implementation
db/migrations/NNN_council.sql            take next free NNN
docs/COUNCIL-PORT.md                     this file
docs/agent/skills/council.md
docs/agent/packets/CIL-01-…md            story stack
```

`NNN` = next integer after the highest file in `db/migrations/` at implement
time. Do not guess. `ls db/migrations | tail`.

## 3. Submodule install (operator)

Run from a clean `main` worktree, scheduler stopped
(`pnpm agent:scheduler:stop`, `pnpm forge:doctor` shows 0 open engine tasks).

```bash
# 1. Add at a pinned commit, not floating main.
git submodule add https://github.com/0xNyk/council-of-high-intelligence.git \
  vendor/council-of-high-intelligence

cd vendor/council-of-high-intelligence
git fetch --tags
# Prefer the latest annotated tag that the adapter tests still pass.
git checkout v1.2.0    # replace with tag current at implement time
VENDOR_SHA=$(git rev-parse HEAD)
cd ../..

# 2. Record the pin WE own. The submodule SHA in git is the lock;
#    this JSON is what the service reads so a missing checkout fails closed.
cat > vendor/council-pin.json <<EOF
{
  "repo": "https://github.com/0xNyk/council-of-high-intelligence.git",
  "tag": "v1.2.0",
  "sha": "${VENDOR_SHA}",
  "adapterVersion": "1.0.0",
  "protocolFiles": [
    "SKILL.md",
    "agents"
  ],
  "ignored": [
    "install.sh",
    ".claude-plugin",
    "scripts/council-simulation-checklist.sh"
  ]
}
EOF

# 3. Make clone usable for everyone.
# .gitmodules should contain:
#   [submodule "vendor/council-of-high-intelligence"]
#     path = vendor/council-of-high-intelligence
#     url  = https://github.com/0xNyk/council-of-high-intelligence.git
#     update = none
# `update = none` stops `git submodule update --remote` from silently
# floating us onto their main.

# 4. Fresh clones
git submodule update --init --recursive
```

Vercel / CI: submodules must be fetched. In Vercel project settings enable
**Include submodules**, or add to the install command:

```bash
git submodule update --init --depth 1 vendor/council-of-high-intelligence
```

If the checkout SHA ≠ `vendor/council-pin.json.sha`, `CouncilService` refuses
to convene (`COUNCIL_VENDOR_DRIFT`). That is the future-update fuse.

## 4. Future updates (the whole point of the pin)

Upstream will move. We do not auto-merge.

```
scripts/council-vendor-sync.sh status     # pin vs checkout vs origin tags
scripts/council-vendor-sync.sh fetch      # fetch tags, do not checkout
scripts/council-vendor-sync.sh propose vX # checkout tag in submodule, print diff
scripts/council-vendor-sync.sh pin        # rewrite council-pin.json to HEAD
scripts/council-vendor-sync.sh verify     # SHA match + protocol files exist
```

Update recipe:

1. Read their `CHANGELOG.md` for the tag. Note roster drift, schema, modes.
2. `propose` the tag. Diff only `SKILL.md` and `agents/*.md`. Ignore brand art.
3. Run adapter tests (`protocol.ts` parses frontmatter; refuse table still
   covers our subject types).
4. If they renamed a polarity pair we mapped, update `services/council/panels.ts`
   in the **same** commit as the pin bump.
5. Bump `adapterVersion` only when *our* verdict schema changes. Vendor tag
   bumps do not change adapter version if the parsed contract is unchanged.
6. Persist `council_vendor_pin` row (singleton update) so old verdicts keep
   the SHA they were born under. New sessions get the new SHA.
7. Never rewrite historical `council_verdict` rows to the new protocol.

If upstream deletes a persona we mapped, the adapter must fail closed on that
panel until we remap. Do not silently substitute Socrates for `frame-break`.

Dependabot on submodules is optional and noisy. Prefer a 30-day calendar
reminder and a Forge paper-intake duo: *pirate vs reject this tag*.

## 5. Domain model

### 5.1 Subject types (closed enum)

| `subject_type` | Domain | Typical pick set |
|---|---|---|
| `deal` | app | accept / counter / hold / walk |
| `offer` | app | accept / counter / reject / wait |
| `match` | app | show / wait / swap / introduce |
| `listing_intake` | app | publish / hold / missing-facts |
| `forge_story` | forge | convene-duo / skip / recut |
| `forge_architect` | forge | freeze-contract / hold-frame |
| `forge_lead_pre` | forge | SOLO / SMITH / HOLD |
| `paper_intake` | forge | pirate / reject / defer |

Adding a subject type is a story. It is not a config flag an LLM may invent.

### 5.2 Verdict (adapter v1)

```
pick                 string, required, one action
reasons              1..3 strings
dissent              >=1 non-overlapping objection  (duo may allow 1)
kill                 >=1 criterion that reverses the pick
next.action          string
next.owner           string
next.reviewAt        timestamptz
evidence[]           { claim, kind: FACT|INFERENCE|ASSUMPTION|UNKNOWN }
outcome              null | confirmed|revised|reversed|inconclusive
```

This is the object Lead PRE, portal cards, and Assay all read. Transcripts
are `raw_model jsonb` and are **not** the fact.

### 5.3 Modes

| Mode | When |
|---|---|
| `refuse` | lookup, cheap experiment, one-writer operational fact, predictable outcome, “bless my answer” |
| `duo` | default for irreversible or frame-risk work |
| `quick` | breadth, no cross-exam, still one verdict |
| `full` | **not implemented.** Service fails `COUNCIL_MODE_UNSUPPORTED`. |

### 5.4 Panels we own

Forge duo: `frame-break` (elenchus, 3-level + hemlock) × `rebuild` (first principles).

Deal desk triad (quick, not 18): `comp` × `buyer-fit` × `fragility`.

Match duo: `buyer-fit` × `listing-facts`.

Intake duo: `measured` × `unknowns`.

`protocol.ts` may *consult* upstream polarity pairs to stay honest about
method names. It must not require Aristotle to exist for the deal desk to run.

## 6. Neon

See `db/migrations/NNN_council.sql`.

Writer audit: only `db/council.ts` touches these tables. Add the pair to
`docs/agent/COLUMN-WRITER-AUDIT.md` in CIL-02.

Promotion: DEV apply + verify, then PROD apply + verify, then
`pnpm db:parity`. Story is not done when the file exists
(AGENTS.md database delivery rule).

## 7. Service surface

Domain `council`, version `1`.

| Operation | Kind | Auth | Notes |
|---|---|---|---|
| `convene` | command | `council.write` | refuse-or-open session; ordered by `subjectId` |
| `recordVerdict` | command | `council.write` | the one writer of the verdict |
| `checkpoint` | command | `council.write` | sets outcome only; does not rewrite pick |
| `getBySubject` | query | `council.read` | latest session + verdict |
| `get` | query | `council.read` | by session id |
| `verifyVendor` | query | `council.read` | pin vs checkout |

`convene` is idempotent on `(subject_type, subject_id)` while a session is
`queued` or `running`. A second call returns the existing session. A new
session is allowed only after `verdict` / `refused` / `failed`.

Model calls go through `CouncilModelPort`. CIL-03 ships a `NullModelPort`
that forces `refuse` or accepts an injected verdict (tests). CIL-05 binds
DeepSeek / OpenCode. The vendor CLI is never the port.

## 8. Forge wiring (later stories, not this file's job to implement)

- Architect `maxSteps` stays 1. The duo is *inside* the single plan call
  via `council.convene` + `recordVerdict`. Findings row stores `session_id`.
- Lead PRE parser in `agent-runtime/lead-decision.ts` grows optional
  `LEAD_DISSENT` / `LEAD_KILL` / `LEAD_NEXT` **or** reads the council
  verdict by story id. Do not create a second adjudicator beside the row.
- Assay `checkpoint` only. No second verdict.
- Packet lint (CIL-07): irreversible stories require a `council_session`
  in `verdict` or an explicit `refused` with reason.

## 9. App wiring (later stories)

Portal and deal/offer/showing/property call `council` through the registry.
They never import `protocol.ts` or the vendor tree.

UI shows pick, dissent, kill, next, evidence kinds. No marble busts.
No “epistemic diversity scorecard.”

## 10. What this port explicitly will not do

- Replace DeepSeek / Pi / Warp / OpenCode harnesses.
- Teach Smith to debate.
- Council last-contact, WhatsApp attribution, or MLS field maps.
- Special-case any listing.
- Float on upstream `main`.
- Let two sessions for one subject run at once.
