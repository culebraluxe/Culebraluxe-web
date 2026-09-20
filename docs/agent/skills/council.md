# Skill — council

Load with the story packet. Do not install upstream `install.sh`.

- Domain folder: `services/council/`
- Writer: `db/council.ts` only
- Pin: `vendor/council-pin.json` must match submodule SHA or convene refuses
- Modes: `refuse` | `duo` | `quick`. `full` is a fail.
- Verdict shape: pick, 1..3 reasons, dissent >=1, kill >=1, next, evidence kinds, later outcome
- Not a Forge lane. Architect / Lead PRE / Assay / deal / match / intake *call* the service
- Do not council last-contact, identity, MLS maps, or marketing tone
- Checkpoint never rewrites pick
- After a vendor tag bump, remap `panels.ts` in the same commit if method names moved
