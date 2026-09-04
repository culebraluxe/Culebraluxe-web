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
- Architect and Inspector are judgment-lab (Grok / human). They are not auto-queued.
- Assay is TUNIT with an allow-list. Empty assay list means Assay does not launch.
- Only `builder` may keep a git commit or write DEV. Scout/Assay commits are rewinded.
- `main` is production-sensitive. Agents commit locally and do not push.
- Code + DEV schema + PROD schema + verification = done for schema stories.
- Do not reset PROD or copy DEV over PROD to fix drift.
- Google Maps is the production map. Never use the Demo key in production.
