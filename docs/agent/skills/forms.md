# Skill: forms

- Listing agreements and PDFs are bound to participants and signatures.
- Existing signed agreements stay immutable unless the story says otherwise.
- Prefer the current form pipeline over a one-off generator.
- Do not leak provider URLs or raw webhook payloads into form metadata.

## Anchored to
- `lib/forms/form-instance-io.ts` — the read/write seam routes import; `legacy/db/form-service-repository.ts` holds the SQL.

