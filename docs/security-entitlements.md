# Portal entitlement map (review draft)

Existing `BaseService` operations and Rust `AuthorizationPort` enforce actions. The navigation registry supplies screen visibility; Yew controls use reducer-owned grants. Hiding UI never authorizes an API request.

| Role | Main site | Portal reads | Portal commands | TECH |
| --- | --- | --- | --- | --- |
| External Guest | Public reads | None | None | None |
| Internal Guest | Public reads | Granted query actions | None | None |
| USER | Public reads | Granted query actions | `person.write`, `showing.write`, `calendar.write`, `form.write` | None |
| Power User | Public reads | All non-TECH query actions | All non-TECH commands | None |
| ROOT | Public reads | All | All | Full |

## Navigation and screen actions

| Live destination | Read action | Write action when applicable |
| --- | --- | --- |
| `/portal/dashboard` | `cockpit.read` | — |
| `/portal/clients` | `person.read` | `person.write` |
| `/portal/projects` | `project.read` | `project.write` |
| `/portal/deals` | `deal.read` | `deal.write` |
| `/portal/documents` | `vault.read` | `vault.write` |
| `/portal/workflows` | `portal.read` | — |
| `/portal/forms` | `form.read` | `form.write` |
| `/portal/core/seller-strategy` | `portal.read` | — |
| `/portal/accounting` | `accounting.read` | `accounting.write` |
| `/portal/accounting/receivables` | `accounting.read` | `accounting.write` |
| `/portal/accounting/expenses` | `accounting.read` | `accounting.write` |
| `/portal/accounting/pnl` | `accounting.read` | `accounting.write` |
| `/portal/accounting/receipt-scanner` | `accounting.read` | `accounting.write` |
| `/portal/marketing` | `property.read` | `property.write` |
| `/portal/marketing/syndication` | `property.read` | `property.write` |
| `/portal/property-admin` | `property.read` | `property.write` |
| `/portal/property-media` | `property.read` | `property.write` |
| `/portal/tech` | `tech.access` | — |
| `/portal/storyboard` | `tech.access` | — |
| `/portal/design-lab` | `tech.access` | — |
| `/portal/system-health` | `portal.read` | — |
| `/portal/db-test` | `portal.read` | — |
| `/portal/admin/whatsapp-meta` | `portal.read` | — |
| `/portal/settings` | `security.principal.read` | `security.entitlement.manage` (ROOT only) |

Rows reflect the listed destinations in `lib/navigation/registry.ts`; detail, retired, and unlisted routes still require their service operation checks. `tech.access` remains a ROOT-only authority. The SUPPORT Security screen displays the active role grants through the Rust Security service.

## Complete portal screen inventory

The existing Rust MVI `SCREENS` registry contains these 59 `/portal/` screens plus the `/portal-auth-proof` diagnostic. Actions for the 24 menu destinations come from `lib/navigation/registry.ts`; the other rows are proposed read actions for screens without a current menu entry, including routes marked `Listed` in the MVI registry but absent from the navigation registry. **Only the 24 menu destinations currently use this map for visibility.** A row here does not mean the page route and every underlying API have been independently gated; those checks must be completed before release.

| MVI screen | Route | Surface | Navigation | Read action |
| --- | --- | --- | --- | --- |
| `dashboard` | `/portal/dashboard` | CORE | listed | `cockpit.read` |
| `clients` | `/portal/clients` | CORE | listed | `person.read` |
| `projects` | `/portal/projects` | CORE | listed | `project.read` |
| `deals` | `/portal/deals` | CORE | listed | `deal.read` |
| `cabinet` | `/portal/documents` | CORE | listed | `vault.read` |
| `workflows` | `/portal/workflows` | CORE | listed | `portal.read` |
| `forms` | `/portal/forms` | CORE | listed | `form.read` |
| `seller-strategy` | `/portal/core/seller-strategy` | CORE | listed | `portal.read` |
| `accounting` | `/portal/accounting` | ACCOUNTING | listed | `accounting.read` |
| `accounting-receivables` | `/portal/accounting/receivables` | ACCOUNTING | listed | `accounting.read` |
| `accounting-expenses` | `/portal/accounting/expenses` | ACCOUNTING | listed | `accounting.read` |
| `accounting-pnl` | `/portal/accounting/pnl` | ACCOUNTING | listed | `accounting.read` |
| `accounting-receipt-scanner` | `/portal/accounting/receipt-scanner` | ACCOUNTING | listed | `accounting.read` |
| `marketing` | `/portal/marketing` | MARKETING | listed | `property.read` |
| `marketing-syndication` | `/portal/marketing/syndication` | MARKETING | listed | `property.read` |
| `issues` | `/portal/issues` | OPS | listed | `portal.read` |
| `needs-review` | `/portal/needs-review` | OPS | listed | `portal.read` |
| `property-admin` | `/portal/property-admin` | OPS | listed | `property.read` |
| `media-admin` | `/portal/media-admin` | OPS | listed | `property.read` |
| `property-media` | `/portal/property-media` | OPS | listed | `property.read` |
| `identity-quality` | `/portal/identity-quality` | OPS | listed | `person.read` |
| `client-admin` | `/portal/client-admin` | OPS | listed | `person.read` |
| `reporting` | `/portal/reporting` | OPS | listed | `portal.read` |
| `decision-analysis` | `/portal/decision-analysis` | OPS | unlisted | `portal.read` |
| `system-health` | `/portal/system-health` | SUPPORT | listed | `portal.read` |
| `db-test` | `/portal/db-test` | SUPPORT | listed | `portal.read` |
| `whatsapp-meta` | `/portal/admin/whatsapp-meta` | SUPPORT | listed | `portal.read` |
| `security` | `/portal/settings` | SUPPORT | listed | `security.principal.read` |
| `settings-authorities` | `/portal/settings/authorities` | SUPPORT | unlisted | `security.principal.read` |
| `settings-roles` | `/portal/settings/roles` | SUPPORT | unlisted | `security.principal.read` |
| `settings-users` | `/portal/settings/users` | SUPPORT | unlisted | `security.principal.read` |
| `whatsapp-coexistence` | `/portal/admin/whatsapp-coexistence` | TECH | unlisted | `tech.access` |
| `attention` | `/portal/attention` | CORE | unlisted | `cockpit.read` |
| `activity` | `/portal/activity` | CORE | unlisted | `cockpit.read` |
| `showings` | `/portal/showings` | CORE | unlisted | `showing.read` |
| `tech` | `/portal/tech` | TECH | listed | `tech.access` |
| `storyboard` | `/portal/storyboard` | TECH | listed | `tech.access` |
| `design-lab` | `/portal/design-lab` | TECH | listed | `tech.access` |
| `framer-ui-lab` | `/portal/tech/framer-ui-lab` | TECH | unlisted | `tech.access` |
| `media-test` | `/portal/media-test` | TECH | unlisted | `tech.access` |
| `rust-lab` | `/portal/tech/rust-lab` | TECH | listed | `tech.access` |
| `tech-lab` | `/portal/tech/lab` | TECH | listed | `tech.access` |
| `command-center` | `/portal/command-center` | TECH | retired | `tech.access` |
| `command-console` | `/portal/command-console` | TECH | retired | `tech.access` |
| `tech-grok` | `/portal/tech/grok` | TECH | retired | `tech.access` |
| `tech-flight-recorder` | `/portal/tech/flight-recorder` | TECH | retired | `tech.access` |
| `tech-app-errors` | `/portal/tech/app-errors` | TECH | unlisted | `tech.access` |
| `tech-runs` | `/portal/tech/runs` | TECH | unlisted | `tech.access` |
| `tech-kanban` | `/portal/tech/kanban` | TECH | unlisted | `tech.access` |
| `tech-line` | `/portal/tech/line` | TECH | unlisted | `tech.access` |
| `client-record` | `/portal/clients/[personId]` | CORE | record | `person.read` |
| `deal-record` | `/portal/deals/[dealId]` | CORE | record | `deal.read` |
| `form-record` | `/portal/forms/[formId]` | CORE | record | `form.read` |
| `workflow-record` | `/portal/workflows/[instanceId]` | CORE | record | `portal.read` |
| `property-record` | `/portal/property-admin/[propertyId]` | OPS | record | `property.read` |
| `story-record` | `/portal/storyboard/[id]` | TECH | record | `tech.access` |
| `trace-record` | `/portal/tech/flight-recorder/[instanceId]` | TECH | unlisted | `tech.access` |
| `runtime-record` | `/portal/runtime-inspector/[instanceId]` | SUPPORT | unlisted | `portal.read` |
| `portal-auth-proof` | `/portal-auth-proof` | SUPPORT | unlisted | `portal.read` |
| `console-story` | `/portal/command-console/[storyId]` | TECH | record | `tech.access` |

## Delivery status

- The Rust authorization port, role grant migration, navigation projection, and selected Yew controls are staged for verification. The existing TypeScript service kernel and Forms binding use the same role grant tables while they remain in use.
- The SUPPORT editor changes active internal-role grants through the Security service. The canonical Rust authorization port requires the exact `root` role for `security.entitlement.manage`; view and reducer controls use the same exact-root projection. Authorized mutation outcomes are audited by the Security service. Authorization refusals fail before mutation. Showing commands within a Deal workspace use `showing.write`; other Deal workspace commands use `deal.write`.
- DEV migrations 210 and 211 are applied and verified. Migration 212 is a forward-only correction that removes the `owner` management grant seeded by 211; it must be applied on DEV before this slice is closed. Migration 210 contributes 33 base actions / 208 grants; 211 adds the management action and two grants; after 212 the intended state is 34 actions / 209 grants.
- Rust workspace compilation is green and the committed `rust/Cargo.lock` includes Casbin. The repository-wide rustfmt and TypeScript lint backlogs predate this entitlement slice; this slice must not add new debt.
- PROD remains untouched. Migrations 210-212 must be applied and checked on PROD only as a separate explicit release step.
