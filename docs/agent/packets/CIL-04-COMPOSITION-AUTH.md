# CIL-04 — register council in the kernel

## Why

A service that is not in `composeCoreServices` is a second kernel.

## Units

- **A** — Add `council?: CouncilRepository` to `CoreServiceRepositories` and
  register `CouncilService` when present, same optional style as `comms`.
- **B** — Production composition always supplies the repo.
- **C** — Entitlement actions `council.read` / `council.write`. Fail closed.
  Do not ship an open-stub for write.
- **D** — One line on `docs/agent/MAP-services.md`.
- **E** — Error capture through existing `ServiceErrorSink`. No new sink.

## Acceptance

- `registry.dispatch('council', envelope)` works in the full runtime.
- Missing authorization port still fails as BaseService already does.
- Showing/person/property composition tests still pass.

## Not this story

- No HTTP route required if an existing MVI command bus can carry envelopes.
- If a route is added, it is `/api/portal/council/*` behind portal auth, using
  `withApiHandler`, and it only calls the service.
