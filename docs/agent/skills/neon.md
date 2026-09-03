# Skill: neon

- Canonical data lives in Postgres on Neon. Do not invent a parallel store.
- Migrations live in `db/migrations` and must be numbered.
- Repository modules own driver-value normalization.
- Prefer a Neon *branch* for dangerous schema experiments. Never reset PROD.
- WhatsApp is a channel, not an identity type. Phones are strict E.164.
- A story that touches schema is not done until DEV and PROD match the code.
- Install Neon vendor skills for CLI/branch syntax if needed; do not spawn a Neon-hosted agent as Smith.
