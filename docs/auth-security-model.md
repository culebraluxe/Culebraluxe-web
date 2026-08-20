# CulebraLuxe Application Security Model

Status: AUTH-01 foundation (schema). Not yet enforced at runtime.

Source of truth: `db/migrations/015_auth_security_model.sql`.

## The model

```
authenticated identity → app_user → role → authority
```

| Concept | Meaning | Owned by |
|---------|---------|----------|
| **AUTHENTICATION** | Proves *who signed in*. | A future auth provider/session layer (not yet implemented). |
| **APP_USER** | The canonical application actor. May be `internal` or `external`. Optionally links to a CRM `person`. | `app_user` |
| **ROLE** | A human-meaningful named bundle of authorities, scoped to an account type. | `role` |
| **AUTHORITY** | A coarse application capability ("may this actor attempt this class of command?"). NOT a per-button/per-function entitlement. | `authority` |
| **DOMAIN / CRM-14** | Decides whether a particular *business* action is legal in the current deal/listing state. | Domain/workflow services (future CRM-14). |

Authorization answers *"May this actor attempt this class of command?"*.
It does **not** answer *"Is this business transition legal in the current deal state?"* — that stays with domain/workflow services.

## CRM `person` is not the authentication principal

- `person` is the CRM relationship record (buyers, sellers, clients, contacts).
- `app_user` is the application actor.
- An `app_user` may optionally link to an existing `person` via `app_user.person_id` (nullable FK).
- External users may link to their canonical CRM person; internal users need not.
- No automatic person creation and no email-based runtime matching are performed here.

## Account types

`app_user.account_type` and `role.account_type` are one of:

- `internal` — brokerage staff (`owner`, `agent`, `viewer`).
- `external` — customer/client accounts (`client`).

Cross-type assignment is blocked at the database boundary: a single `BEFORE INSERT OR UPDATE` trigger
(`enforce_app_user_role_account_type`) on `app_user_role` compares the role's account type to the
app_user's account type and raises on mismatch. System Health also exposes a read-only
account-type-mismatch count as a safety net.

## Future examples

- **Internal viewer** — an `internal` app_user with the `viewer` role → `portal.read`, `deal.read`.
- **Internal agent** — an `internal` app_user with the `agent` role → `portal.read`, `crm.write`, `listing.write`, `deal.read`, `deal.write`.
- **External client** — an `external` app_user (optionally linked to a CRM person) with the `client` role → `external.properties.save`, `external.deal.read_own`.

## Authority list (coarse)

`portal.read`, `crm.write`, `listing.write`, `deal.read`, `deal.write`,
`settings.read`, `settings.manage`, `external.properties.save`, `external.deal.read_own`.

## Security boundary statements

- Application users never receive Neon/Postgres credentials.
- Database access remains server-side only, through application connection/pool credentials.
- Authentication identity is not database identity.
- A provider-specific authentication subject (e.g., an OAuth/OIDC subject) will eventually be mapped to an `app_user`; the `app_user` then carries roles/authorities.
