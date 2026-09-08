# Decision log

Short facts that are expensive to rediscover. Not the current story — that is the packet.

- WhatsApp is an interaction *channel*, not a person-identity type. Actors resolve through strict E.164 `phone`.
- Story Board + `agent_work_item` is the only coding queue. Ready is authorization.
- One Claimed/Running item system-wide (for now). One active item per story always.
- Lanes are job shapes (Scout → Architect → Lead → Smith 1..N → QA → DEV_OPS), not department experts.
- QA contains Candidate Assay as an operation: exact-SHA PASS/FAIL math, prose never overrides arithmetic.
- Inspector is a QA independent-review capability (lineage separation), not a seventh agent. Archive is a capability; night is a Smith grade.
- Six Forge roles: Scout (what is going on?) → Architect (what should we build/change?) → Lead (how do we get this story done?) → Smith (can I build it correctly?) → QA (is it correct and ready to ship?) → DEV_OPS (can I safely get it into production and prove it?).
- Spend vision: team map owns model selection; forge-native runs pin exact models via dsh --patch; model_used persists per run (107); Lead PRE decides grades against relative prices.
- Skills are markdown packs. Neon skill packs are knowledge. Neon Functions are not Smith.
- Architect and Inspector are judgment-lab labels. They auto-run overnight so Smith can produce code. Humans review a working SHA in the morning pack, they do not sit on an empty Architect gate.
- Assay is TUNIT with an allow-list. Empty assay list means Assay does not launch.
- Only `builder` may keep a git commit or write DEV. Scout/Assay commits are rewinded.
- `main` is production-sensitive. Agents commit locally and do not push.
- Code + DEV schema + PROD schema + verification = done for schema stories.
- Do not reset PROD or copy DEV over PROD to fix drift.
- Google Maps is the production map. Never use the Demo key in production.
- Errors are captured durably, never silently: DB (gateway) + service kernel (`ServiceErrorSink` via `composeCoreServices`/`appServiceErrorSink`) + route/action seams (`withApiHandler`/`withServerErrorCapture`/`captureServerError`). Expected business outcomes (validation, FORBIDDEN, not-found) are audited control flow, never error rows. See AGENTS.md "Error Capture Obligation". New code that fails without the framework is a review reject.
- FORGE_ROUTING_BRAIN defaults to reducer. engine mode skips hydrate/follow/publish and drives FORGE_SDLC. Never dual-write.
- ENG-FORGE-V5-21 (captured for the NEXT project, do not build this cycle): Cline beat Forge on real tasks because Cline keeps ONE continuous working context while Forge gave each lane amnesia (Scout→Architect→Lead→Smith→Repair each got a fresh model session reconstructing the problem). Fix = STORY-RESIDENT OpenCode session continuity: resume the SAME story-bound OpenCode session across nodes so context persists, but Forge reasserts role/authority per lane — "context persists; authority does not." Candidate freeze is a HARD CONTEXT WALL: Reviewer/QA must be a fresh independent context, and Deterministic Assay stays the final authority. Open risks to prove before trusting: resumed-session role permission retention; model-switch-on-resume bugs; Forge must persist the opaque OpenCode session/task id; NO session reuse across stories/worktrees; candidate freeze terminates the shared construction context. Tooling helps agents operate better but does NOT fix this underlying cognitive handicap.
- Cost/forecast units are Forge WIDGETS (model weight × minutes, normalized, model-relative), NOT USD and NOT tokens. Tokens = provider metering. USD is reserved for actual settlement only. A future USD/widget spot-rate conversion layers on later (per-model rate × tokens × $). Widgets make the model a replaceable component: swap provider/adapter + rate row, never the workflow/gates/actuals. Schema: `storyboard_story_run.cost_widgets` (+`cost_source`) and `work_estimate.estimated_widgets`; `cost_usd` stays null unless real vendor spend.
- ENG-FORGE-PHASE-AGENT (design, parked — see docs/agent/packets/ENG-FORGE-PHASE-AGENT.md): abstract `ForgePhaseAgent` owns the role/phase lifecycle + an ENFORCED deliverable gate (assertDeliverable), layered over the existing AgentRuntimeAdapter execution base. Subclasses (Scout/Architect/Lead/Smith/QA/DevOps) declare their deliverable; a role cannot Complete without it (else HOLD/retry). Generalizes the three marshaling fixes (scout context_refs packet, architect research_disposition, findings parse). Hosts V5-21 context continuity + error capture.
- DEV hygiene: run `APP_ENV=development node --env-file=.env.local scripts/forge-cleanup-dev.ts --apply` (dry-run default) to prune orphaned engine rows + stale worktrees so accumulated garbage never interferes with real runs.
