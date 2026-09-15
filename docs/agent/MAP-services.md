# MAP — services (where domain logic lives)

Every folder under `services/` is one domain. The shape inside each is deliberately the same:

- `<domain>-service.ts` — the business rules; the file to open first,
- `repository.ts` — the only place that touches the database for this domain,
- `index.ts` — the public surface other layers import.

Above them, `services/composition.ts` wires services together; `services/core/base-service.ts` and
`services/core/role.ts` hold the shared kernel (and the `ServiceErrorSink` that captures unhandled
service errors). Application code talks to `services/`, never straight to a database client.

| Domain | Path | What it is for |
|---|---|---|
| property | `services/property/` | The canonical listing: property records, their facts, and property↔media relationships |
| person | `services/person/` | People and identity resolution (actors, contacts); identity keys live here, not in channels |
| comms | `services/comms/` | Interaction channels (messages/email/WhatsApp intake) — a channel is not an identity |
| firm | `services/firm/` | Firm-level records — open `firm-service.ts` for its contract (one line here would be a guess) |
| contract | `services/contract/` | Contracts and their terms, including amendment/term-delta work |
| media | `services/media/` | The reusable media asset (image / video / document) and its roles |
| forms | `services/forms/` | The forms domain behind the Forms PDF composer |
| project | `services/project/` | Projects and playbooks (project-scoped work) |
| showing | `services/showing/` | Property showings and their scheduling |
| calendar | `services/calendar/` | Calendar sync and events (Apple-forward; see the calendar notes in `MEMORY.md`) |
| wbs | `services/wbs/` | Work-breakdown structures and categories |
| vault | `services/vault/` | Storage/vault domain — `types.ts` states its contract |
| security | `services/security/` | Security levels and their repository |
| entitlement | `services/entitlement/` | Authorization: entitlement service + the DB-backed authorization policy provider |
| regrid | `services/regrid/` | Parcel/Regrid CSV import (has its own `README.md` — read it before touching the CSV parser) |
| core | `services/core/` | The shared kernel: `base-service.ts`, `role.ts`, composition helpers |

**How to use this map:** find the domain, open `<domain>-service.ts`, then `repository.ts` for the data
shape. If a feature spans domains, the seam is `services/composition.ts`. If you cannot find a domain for
what you are doing, that is a design question — ask before inventing a folder (AGENTS.md: extend existing
abstractions before inventing parallel systems).
