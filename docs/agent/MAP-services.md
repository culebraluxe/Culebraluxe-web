# MAP - services (where the domain lives, and how to add one)

A **service** is a Rust struct that holds the rules for one domain. It lives in `rust/server/src/<domain>/mod.rs`, or in
`rust/server/src/<domain>.rs` when it is small. It is typed over a **repository trait**, so a test can hand it a fake
instead of a database, and it is constructed over a **DAO** (`rust/core/db/src/<domain>.rs`), which is the only file
that runs SQL for that domain. It becomes reachable only when it is registered in `rust/server/src/composition.rs`.

The short version of where a new rule goes:

| You are writing | It goes in |
| --- | --- |
| A type, a validation, an invariant | `rust/core/domain/src/<domain>.rs` |
| A query or a write | `rust/core/db/src/<domain>.rs` (`<Domain>Dao`) |
| An operation with authorization and audit | `rust/server/src/<domain>/mod.rs` (`<Domain>Service`) |
| A URL | `rust/server/src/api/routes.rs` (or `portal_bridge.rs` for a portal screen read) |
| The wiring that makes it dispatchable | `rust/server/src/composition.rs` |

`rust/server/src/issues.rs` is the whole shape in 72 lines. Read it before writing a new service; the recipe below is
that file, generalized.

## What the kernel gives a service for free

`rust/core/service/` is the kernel; `rust/server/src/service_support.rs` is how a server-side service uses it. A service
does not fetch identity, decide authorization, write audit rows, publish events or capture failures by itself - it asks
the runtime:

- **Identity** - `ServiceContext` (actor, principal, correlation id, `OperationKind`). Built per request by
  `rust/server/src/api/context.rs`, never inside a service.
- **Authorization** - `authorize(&runtime, domain, action, operation, kind, context)` returns a decision, or
  `FORBIDDEN`. A refusal is written to audit and is **not** an error row: it is control flow.
- **Audit** - `audit_result(...)` writes exactly one row per operation with its outcome and its error code, success or
  failure. Call it on both paths; it reads the outcome off the `Result` you hand it.
- **Domain events** - `DomainEventPort` (transactional outbox) for facts other domains react to.
- **Failures** - `CoreServiceError`: `Business { code, message }` for a domain refusal, `Database` from a `DbFailure`,
  `Runtime` for a port that failed. Unhandled failures are routed into `app_error` by the kernel's error sink.
- **Lifecycle** - one mailbox per domain, `warm_cache()` before traffic, drain on shutdown. A call arriving during a
  drain fails with `ServiceDispatchError::ServiceDraining` rather than being served by half a service.
- **Bounded execution** - a route runs a service method through the registered mailbox (`execute_registered`), so the
  queue, the timeout and the refusals apply to every door: HTTP, the engine and MQ alike.

## How to add a service - the recipe

**1. The types.** Add them to `rust/core/domain/src/<domain>.rs` and export from `rust/core/domain/src/lib.rs`. Types
carry `serde` and, where they cross a boundary, a validating constructor. No SQL, no HTTP, no provider client here.

**2. The DAO.** `rust/core/db/src/<domain>.rs`:

```rust
#[derive(Clone)]
pub struct WidgetDao { db: Database }

impl WidgetDao {
    pub fn new(db: Database) -> Self { Self { db } }

    pub async fn page(&self, scope: &str) -> DbResult<WidgetsPage> {
        let rows = sqlx::query_as::<_, WidgetRow>("select ... from widget where kind = $1")
            .bind(scope)
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("widget.page", &error))?;
        Ok(rows.into_iter().map(WidgetRow::into_page).collect())
    }
}
```

- One pool per process: `self.db.pool()` - never build one in the DAO.
- Bind parameters. Never string-build SQL, never hand-escape. A `uuid` column needs `$1::uuid`.
- Return `DbResult<T>` so a failure keeps its incident id; do not `unwrap` a query.
- Reads that can be retried go through the `db::retrying_read!` macro (used in `rust/server/src/issues.rs`).
- Export the DAO from `rust/core/db/src/lib.rs`.

**3. The service.** `rust/server/src/<domain>/mod.rs`, with the repository trait as the test seam:

```rust
#[async_trait]
pub trait WidgetRepository: Send {
    async fn page(&self, scope: &str) -> DbResult<WidgetsPage>;
}

#[async_trait]
impl WidgetRepository for WidgetDao {
    async fn page(&self, scope: &str) -> DbResult<WidgetsPage> {
        db::retrying_read!(WidgetDao::page(self, scope))
    }
}

pub struct WidgetService<R> { repository: R, runtime: ServiceRuntime }

impl<R: WidgetRepository> WidgetService<R> {
    pub fn new(repository: R, infrastructure: ServiceInfrastructure) -> Self {
        Self { repository, runtime: ServiceRuntime::new(infrastructure) }
    }

    pub async fn page(&self, scope: &str, context: &ServiceContext) -> Result<WidgetsPage, CoreServiceError> {
        const OP: &str = "widget.page";
        let decision = authorize(&self.runtime, "widget", "portal.read", OP, OperationKind::Query, context).await?;
        let result = self.repository.page(scope).await.map_err(Into::into);
        audit_result(&self.runtime, "widget", OP, context, decision, &result).await?;
        result
    }
}
```

The order is fixed: **authorize, do the work, audit.** A service that writes publishes its domain event through the
runtime as well. Refuse with `CoreServiceError::business("WIDGET_LOCKED", "...", false)` (code, message, retryable) when
the user should read the reason; never turn a failure into an empty result and never swallow a `Result`.

**4. Register it - five places, all in `rust/server/src/composition.rs`.**

1. a field on `ServiceCatalog` (`widget: Arc<WidgetService<WidgetDao>>`),
2. its construction in `ServiceCatalog::new` - `WidgetDao::new(db.clone())` plus `infrastructure.clone()`,
3. one line inside `catalog_accessors!`, which generates `pub fn widget(&self) -> Arc<WidgetService<WidgetDao>>`,
4. one line `abstract_service!(WidgetService<WidgetDao>, "widget", "Widget service");` - this is what gives the service
   its domain name, its operation descriptors and its `AbstractService` implementation,
5. push it into `registrations()`, the vector the kernel turns into the registry.

`registrations()` **is** the service map: every domain in production is one line of that vector. A service that is not
there still compiles; its route fails at runtime with `ServiceDispatchError::ServiceNotFound(domain)` mapped to the
kernel's typed failure. That is the symptom of a forgotten step 5.

The list of domains the HTTP surface is expected to reach is itself a test:
`every_http_service_family_maps_to_its_registered_mailbox` in `rust/server/src/api/routes.rs` (it pairs each URL family
with the domain it must dispatch to). Add your route to it.

**5. The route.** In `rust/server/src/api/routes.rs`, mount it on the router and call it through the mailbox:

```rust
async fn widgets(State(state): State<ApiState>, headers: HeaderMap) -> Result<Json<ApiSuccess<WidgetsPage>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().widget();
    let context = resolved.service.clone();
    let value = execute_registered(&state, "widget", "widget.page", json!({}), async move {
        service.page("all", &context).await
    })
    .await
    .map_err(|error| correlate(error, &resolved))?;
    Ok(success(value, &resolved))
}
```

Identity comes from `resolve_request_context`. The domain and operation strings in `execute_registered` must match step 4,
or the call is refused as unregistered. The user sees one of the kernel's typed failures - do not invent a status code
inside a service.

**6. A portal screen read.** Portal screens read one page at a time
(`/api/portal/rust-ui/page?screen=<screen>`). Add the field to the page payload in
`rust/server/src/api/portal_bridge.rs`, add it to `PortalPage` in `rust/ui/src/model.rs`, and let the screen pick it (see
`UI-SCREEN-ARCHITECTURE.md`; `rust/ui/src/app/screens/db_test.rs` is the smallest complete read).

**7. Verify.**

```sh
cargo check --workspace --all-targets
cargo test -p db -p server
pnpm db:migrations            # if anything touched schema
```

Then, for anything that talks to a real database - which is the only way these three bugs were ever caught - run the
live check against DEV: `scripts/rust-live-check/`.

## Testing a service without a database

The repository trait exists for this. A test implements `WidgetRepository` for an in-memory fake and asserts the
service's decisions: which authorization it demanded, what it did on a refusal, what it audited. The kernel's lifecycle
is tested once for everyone in `rust/server/tests/service_harness_dev.rs`, which builds services with
`ServiceHarness::isolated` - a new service does not need to re-prove the kernel.

## Which services exist

The authoritative list is `registrations()` in `rust/server/src/composition.rs`. The ones you will meet first:

| Domain | Service | DAO |
| --- | --- | --- |
| `cockpit` | `rust/server/src/cockpit/mod.rs` | `rust/core/db/src/cockpit.rs` |
| `deal` | `rust/server/src/deals/mod.rs` | `rust/core/db/src/deal_portal.rs` |
| `issue` | `rust/server/src/issues.rs` | `rust/core/db/src/issue.rs` |
| `task` | `rust/server/src/task/mod.rs` | `rust/core/db/src/task.rs` |
| `property` | `rust/server/src/properties/mod.rs` | `rust/core/db/src/property.rs` |
| `client` | `rust/server/src/clients/mod.rs` | `rust/core/db/src/client.rs` |
| `person` | `rust/server/src/people/mod.rs` | `rust/core/db/src/person.rs` |
| `media` | `rust/server/src/media/mod.rs` | `rust/core/db/src/media.rs` |
| `forms` | `rust/server/src/forms/mod.rs` | `rust/core/db/src/forms.rs` |
| `vault` | `rust/server/src/vault/mod.rs` | `rust/core/db/src/vault.rs` |

Two registration doors exist. Most services take the `abstract_service!` macro in `composition.rs`. Seven older, larger
domains (`calendar`, `person`, `firm`, `contract`, `property`, `client`, `security`) carry a hand-written
`impl AbstractService for ...` in `rust/server/src/service_gateway.rs`, and two more (`CommsService`, `ProjectServiceHost`)
do the same in `composition.rs`. **New services use the macro door**; the hand-written ones are history, not a pattern to
copy.

**If no domain fits what you are doing, that is a design question.** Extend the closest existing service rather than
adding a folder (AGENTS.md: extend existing abstractions before inventing parallel systems).