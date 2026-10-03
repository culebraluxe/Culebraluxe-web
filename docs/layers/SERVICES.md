# Layer: SERVICES

Crate: **`web`**. **Owns:** HTTP, identity, authorization, audit, error responses. It is the only way into the
domain, and the only place that knows about HTTP.

## Files that matter

| file | what it is |
| --- | --- |
| `src/api/routes.rs` | the router and every handler. One handler per endpoint; it resolves context, calls a service, wraps the answer. |
| `src/api/context.rs` | identity. `resolve_request_context` (a person, or refuse) and `resolve_engine_context` (a person **or** a background System actor). |
| `src/api/error.rs` | `ApiError` and its one conversion to a response — where uncaptured 5xx responses are recorded. |
| `src/api/error_capture.rs` | the `app_error` sink, plus the panic hook. |
| `src/security/` | the security service and `identity_cache.rs`. |
| `src/composition.rs` | where services are built from the pool: the only place that knows how to assemble them. |
| `src/<area>/mod.rs` | one service per area (clients, properties, projects, …) with its repository trait. |

## The path every request takes

```
route  →  resolve context  →  service  →  repository  →  DAO  →  pool
          (identity, 401/403)   (authorize, work, audit_result)
```

A service method, in order: `authorize(...)` → do the work through its repository → `audit_result(...)`. Policy lives in
the service; the route does not decide anything.

## Two doors, one implementation (the rule — 2026-09-28)

**HTTP handlers call the area service's method directly.** Authorization and audit happen inside that method, so every
caller gets them. **Envelope dispatch** (`ServiceEnvelope` → `AbstractService::dispatch`, the `/v1/services/dispatch`
route, the CLI's `service dispatch`, MQ and the engine) **is for the engine, the mailbox and MQ** — never a second HTTP
path to an operation a route already serves. Both doors must reach the SAME service method; a door that re-implements
an operation is a defect.

`execute_registered` wraps a direct call in the domain's mailbox (bounded queue, timeout, drain refusal). Use it where
that bound matters — today `cockpit.snapshot`, `workflow.list`, `workflow.detail`, `flight-recorder.transaction` and
`task.complete` — not as a ritual on every read. A service whose descriptor lists capabilities but does not implement
`dispatch` (clients, for one) is reachable only through its routes: its envelope door refuses, which is correct.

**No handler touches a DAO.** A handler resolves its context and calls a service method; the only DAO use under
`server/src/api/` is `error_capture.rs`, the failure-capture seam itself. The Client service's audit domain is `clients`
while its descriptor is `client` — deliberately, so the audit history keeps one name (`MEMORY.md`, 2026-09-28).

## Invariants

1. **Every route resolves a context first.** No handler is anonymous. The internal key is required on all of them.
2. **Identity is resolved, never trusted.** A provider identity that is unmapped or inactive is refused (403). The
   browser does not tell this service who it is.
3. **Engine commands may be background.** With identity headers they are attributed to a person; without them they are a
   `System` actor. One header without the other is refused.
4. **5xx is captured, 4xx is not.** A validation failure or a missing record is audited control flow, not error noise.
   A `DbFailure` already announced itself and is not double-captured.
5. **Identity resolution is cached for 30s**, `Known` results only — caching a "no" would lock someone out. Authorization
   still runs on every request, so the audit trail is unchanged.
6. **Panics are captured**, at level `fatal`, by a process hook. "Impossible" leaves a row.

## Read the code

`src/api/routes.rs` for the shape of a handler, then any one area end to end — `clients` is a good one: service,
repository trait, DAO, and the route that calls it. `src/api/engine.rs` shows the one place where a service is *not*
async (the workflow engine blocks; it runs on a bounded thread pool, and the file explains why).
