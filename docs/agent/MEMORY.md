# Decision log

Short facts that are expensive to rediscover. Not the current story — that is the packet.

- WhatsApp is an interaction *channel*, not a person-identity type. Actors resolve through strict E.164 `phone`.
- Story Board + `agent_work_item` is the only coding queue. Ready is authorization.
- One Claimed/Running item system-wide (for now). One active item per story always.
- Lanes are job shapes (Scout / Smith / Assay), not department experts.
- Skills are markdown packs. Neon skill packs are knowledge. Neon Functions are not Smith.
- Architect and Inspector are judgment-lab (Grok / human). They are not auto-queued.
- Assay is TUNIT with an allow-list. Empty assay list means Assay does not launch.
- Only `builder` may keep a git commit or write DEV. Scout/Assay commits are rewinded.
- `main` is production-sensitive. Agents commit locally and do not push.
- Code + DEV schema + PROD schema + verification = done for schema stories.
- Do not reset PROD or copy DEV over PROD to fix drift.
- Google Maps is the production map. Never use the Demo key in production.
