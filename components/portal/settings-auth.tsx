import type {
  SettingsAuthority,
  SettingsRole,
  SettingsUser,
} from "@/db/settings-auth"
import type { SecurityStatus } from "@/db/auth-status"
import type { BreakGlassReadiness } from "@/lib/auth/break-glass-readiness"

// AUTH-01 read-only Settings views (Stories 9-11). No create/edit/delete yet;
// these render canonical app_user / role / authority data once migration 015
// has been applied.

function typeBadge(type: string) {
  const internal = type === "internal"
  return (
    <span
      className={`rounded-sm px-2 py-1 text-[10px] font-light uppercase tracking-[0.14em] ${
        internal
          ? "bg-[var(--portal-blue-pale)] text-[var(--portal-navy)]"
          : "bg-[var(--portal-neutral-pale)] text-[var(--portal-neutral)]"
      }`}
    >
      {type}
    </span>
  )
}

function PageHeading({
  eyebrow,
  title,
  intro,
}: {
  eyebrow: string
  title: string
  intro: string
}) {
  return (
    <div className="mb-8">
      <p className="text-xs font-light uppercase tracking-[0.28em] text-black/40">
        {eyebrow}
      </p>
      <h1 className="mt-3 font-serif text-4xl font-light leading-[1.1]">
        {title}
      </h1>
      <p className="mt-3 max-w-3xl text-sm font-light leading-6 text-black/50">
        {intro}
      </p>
    </div>
  )
}

function Empty({ children }: { children: React.ReactNode }) {
  return (
    <div className="rounded-sm border border-[var(--portal-border)] bg-white p-10 text-center">
      <p className="text-sm font-light text-black/40">{children}</p>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Story 9 — Users
// ---------------------------------------------------------------------------

export function SettingsUsers({ users }: { users: SettingsUser[] }) {
  return (
    <div>
      <PageHeading
        eyebrow="Settings"
        title="Users"
        intro="Application actors. CRM people are not authentication principals; a user may be linked to a person for relationship context."
      />
      {users.length === 0 ? (
        <Empty>No application users on record.</Empty>
      ) : (
        <div className="overflow-x-auto rounded-sm border border-[var(--portal-border)] bg-white">
          <table className="w-full min-w-[820px] border-collapse">
            <thead>
              <tr className="border-b border-[var(--portal-border)] bg-[var(--portal-blue-pale)]">
                <th className="px-4 py-3 text-left text-[10px] font-light uppercase tracking-[0.14em] text-[var(--portal-blue-gray)]">
                  User
                </th>
                <th className="px-4 py-3 text-left text-[10px] font-light uppercase tracking-[0.14em] text-[var(--portal-blue-gray)]">
                  Account
                </th>
                <th className="px-4 py-3 text-left text-[10px] font-light uppercase tracking-[0.14em] text-[var(--portal-blue-gray)]">
                  Linked person
                </th>
                <th className="px-4 py-3 text-left text-[10px] font-light uppercase tracking-[0.14em] text-[var(--portal-blue-gray)]">
                  Roles
                </th>
                <th className="px-4 py-3 text-left text-[10px] font-light uppercase tracking-[0.14em] text-[var(--portal-blue-gray)]">
                  Active
                </th>
                <th className="px-4 py-3 text-left text-[10px] font-light uppercase tracking-[0.14em] text-[var(--portal-blue-gray)]">
                  Created
                </th>
              </tr>
            </thead>
            <tbody>
              {users.map((user) => (
                <tr
                  key={user.id}
                  className="border-b border-[var(--portal-border)] last:border-b-0 hover:bg-[var(--portal-blue-pale)]/40"
                >
                  <td className="px-4 py-4 align-top">
                    <div className="font-serif text-base font-light">
                      {user.displayName}
                    </div>
                    {user.email && (
                      <div className="mt-1 text-xs font-light text-black/45">
                        {user.email}
                      </div>
                    )}
                  </td>
                  <td className="px-4 py-4 align-top">
                    {typeBadge(user.accountType)}
                  </td>
                  <td className="px-4 py-4 align-top text-sm font-light text-black/60">
                    {user.personName ?? "—"}
                  </td>
                  <td className="px-4 py-4 align-top text-sm font-light text-black/60">
                    {user.roles.length > 0 ? (
                      <div className="flex flex-wrap gap-1">
                        {user.roles.map((role) => (
                          <span
                            key={role}
                            className="rounded-sm bg-black/5 px-2 py-0.5 text-[11px]"
                          >
                            {role}
                          </span>
                        ))}
                      </div>
                    ) : (
                      "—"
                    )}
                  </td>
                  <td className="px-4 py-4 align-top text-sm font-light">
                    {user.active ? (
                      <span className="text-[var(--portal-success)]">Active</span>
                    ) : (
                      <span className="text-[var(--portal-archive)]">Inactive</span>
                    )}
                  </td>
                  <td className="px-4 py-4 align-top text-sm font-light text-black/60">
                    {user.createdAtLabel ?? "—"}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Story 10 — Roles
// ---------------------------------------------------------------------------

export function SettingsRoles({ roles }: { roles: SettingsRole[] }) {
  return (
    <div>
      <PageHeading
        eyebrow="Settings"
        title="Roles"
        intro="Named bundles of authorities. Internal and external roles are kept separate; cross-type assignment is blocked."
      />
      {roles.length === 0 ? (
        <Empty>No roles on record.</Empty>
      ) : (
        <div className="space-y-4">
          {roles.map((role) => (
            <div
              key={role.id}
              className="rounded-sm border border-[var(--portal-border)] bg-white p-6"
            >
              <div className="flex flex-wrap items-start justify-between gap-3">
                <div>
                  <div className="flex flex-wrap items-center gap-2">
                    <h2 className="font-serif text-2xl font-light">{role.name}</h2>
                    <span className="rounded-sm bg-black/5 px-2 py-0.5 text-[11px] font-light text-black/50">
                      {role.code}
                    </span>
                  </div>
                  {role.description && (
                    <p className="mt-1 max-w-2xl text-sm font-light text-black/50">
                      {role.description}
                    </p>
                  )}
                </div>
                <div className="flex items-center gap-2">
                  {typeBadge(role.accountType)}
                  <span className="text-xs font-light text-black/45">
                    {role.active ? "Active" : "Inactive"}
                  </span>
                </div>
              </div>

              <div className="mt-4 grid gap-4 md:grid-cols-2">
                <div>
                  <div className="text-[10px] font-light uppercase tracking-[0.18em] text-[var(--portal-blue-gray)]">
                    Authorities ({role.authorities.length})
                  </div>
                  <div className="mt-2 flex flex-wrap gap-1">
                    {role.authorities.length > 0 ? (
                      role.authorities.map((code) => (
                        <span
                          key={code}
                          className="rounded-sm bg-[var(--portal-blue-pale)] px-2 py-0.5 text-[11px] font-light text-[var(--portal-navy)]"
                        >
                          {code}
                        </span>
                      ))
                    ) : (
                      <span className="text-xs font-light text-black/40">None</span>
                    )}
                  </div>
                </div>
                <div>
                  <div className="text-[10px] font-light uppercase tracking-[0.18em] text-[var(--portal-blue-gray)]">
                    Assigned users
                  </div>
                  <div className="mt-2 text-sm font-light text-black/60">
                    {role.userCount}
                  </div>
                </div>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Story 11 — Authorities
// ---------------------------------------------------------------------------

export function SecurityStatusPanel({
  status,
}: {
  status: SecurityStatus
}) {
  const items: Array<{ label: string; value: number; detail: string }> = [
    {
      label: "Active internal users",
      value: status.activeInternalUsers,
      detail: "Internal accounts currently enabled",
    },
    {
      label: "External users",
      value: status.externalUsers,
      detail: "Customer/client accounts",
    },
    {
      label: "Users with no role",
      value: status.usersWithNoRole,
      detail: "Active actors that would lack authority",
    },
    {
      label: "Users with multiple roles",
      value: status.usersWithMultipleRoles,
      detail: "Actors holding more than one role",
    },
    {
      label: "Mapped auth identities",
      value: status.mappedAuthIdentities,
      detail: "Provider subjects linked to app_users",
    },
    {
      label: "Unmapped app_users",
      value: status.unmappedAppUsers,
      detail: "Application users with no provider identity",
    },
    {
      label: "Owner assignments",
      value: status.ownerRoleAssignments,
      detail: "Current owner-role holders",
    },
    {
      label: "Inactive users w/ active role",
      value: status.inactiveUsersWithActiveRoleMappings,
      detail: "Disabled actors still mapped to an active role",
    },
    {
      label: "Account-type mismatches",
      value: status.accountTypeMismatchCount,
      detail: "Should always be 0 (invariant)",
    },
  ]

  return (
    <section className="mt-6 rounded-sm border border-[var(--portal-border)] bg-white p-6">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div>
          <h2 className="font-serif text-2xl font-light">Security Status</h2>
          <p className="mt-1 text-xs font-light text-black/40">
            Operational security facts — no tokens or credentials.
          </p>
        </div>
        <span
          className={`rounded-sm px-3 py-1.5 text-[10px] font-light uppercase tracking-[0.14em] ${
            status.ownerRoleAssignments === 0
              ? "bg-[var(--portal-archive)]/10 text-[var(--portal-archive)]"
              : "bg-[var(--portal-blue-pale)] text-[var(--portal-navy)]"
          }`}
        >
          {status.ownerRoleAssignments === 0
            ? "No owner assigned"
            : "Owner assigned"}
        </span>
      </div>

      <div className="mt-6 grid gap-4 md:grid-cols-3">
        {items.map((item) => (
          <div
            key={item.label}
            className="rounded-sm border border-[var(--portal-border)] bg-[var(--portal-blue-pale)]/30 p-4"
          >
            <div className="text-[10px] font-light uppercase tracking-[0.16em] text-black/45">
              {item.label}
            </div>
            <div className="mt-2 font-serif text-2xl font-light">{item.value}</div>
            <div className="mt-1 text-xs font-light text-black/40">
              {item.detail}
            </div>
          </div>
        ))}
      </div>
    </section>
  )
}

export function BreakGlassStatusPanel({
  readiness,
}: {
  readiness: BreakGlassReadiness
}) {
  const items: Array<{ label: string; ready: boolean }> = [
    { label: "Break-glass configured", ready: readiness.configured },
    { label: "Break-glass enabled", ready: readiness.enabled },
    { label: "Root user resolvable", ready: readiness.rootResolvable },
    { label: "Root user active", ready: readiness.rootActive },
    { label: "Root holds owner role", ready: readiness.ownerRolePresent },
    { label: "Security audit table", ready: readiness.auditTableAvailable },
  ]

  return (
    <section className="mt-6 rounded-sm border border-[var(--portal-border)] bg-white p-6">
      <h2 className="font-serif text-2xl font-light">Break-glass readiness</h2>
      <p className="mt-1 text-xs font-light text-black/40">
        Emergency root access posture. No secrets, hashes, or tokens shown.
      </p>
      <div className="mt-6 grid gap-4 md:grid-cols-3">
        {items.map((item) => (
          <div
            key={item.label}
            className="rounded-sm border border-[var(--portal-border)] bg-[var(--portal-blue-pale)]/30 p-4"
          >
            <div className="text-[10px] font-light uppercase tracking-[0.16em] text-black/45">
              {item.label}
            </div>
            <div
              className={`mt-2 text-sm font-light ${
                item.ready ? "text-[var(--portal-success)]" : "text-[var(--portal-archive)]"
              }`}
            >
              {item.ready ? "Ready" : "Not configured"}
            </div>
          </div>
        ))}
      </div>
    </section>
  )
}

export function SettingsAuthorities({
  authorities,
}: {
  authorities: SettingsAuthority[]
}) {
  return (
    <div>
      <PageHeading
        eyebrow="Settings"
        title="Authorities"
        intro="Coarse application capabilities. Business-state legality is decided by domain/workflow services, not by these."
      />
      {authorities.length === 0 ? (
        <Empty>No authorities on record.</Empty>
      ) : (
        <div className="overflow-x-auto rounded-sm border border-[var(--portal-border)] bg-white">
          <table className="w-full min-w-[760px] border-collapse">
            <thead>
              <tr className="border-b border-[var(--portal-border)] bg-[var(--portal-blue-pale)]">
                <th className="px-4 py-3 text-left text-[10px] font-light uppercase tracking-[0.14em] text-[var(--portal-blue-gray)]">
                  Code
                </th>
                <th className="px-4 py-3 text-left text-[10px] font-light uppercase tracking-[0.14em] text-[var(--portal-blue-gray)]">
                  Name
                </th>
                <th className="px-4 py-3 text-left text-[10px] font-light uppercase tracking-[0.14em] text-[var(--portal-blue-gray)]">
                  Description
                </th>
                <th className="px-4 py-3 text-left text-[10px] font-light uppercase tracking-[0.14em] text-[var(--portal-blue-gray)]">
                  Roles
                </th>
              </tr>
            </thead>
            <tbody>
              {authorities.map((authority) => (
                <tr
                  key={authority.id}
                  className="border-b border-[var(--portal-border)] last:border-b-0 hover:bg-[var(--portal-blue-pale)]/40"
                >
                  <td className="px-4 py-4 align-top">
                    <span className="rounded-sm bg-[var(--portal-blue-pale)] px-2 py-0.5 text-[11px] font-light text-[var(--portal-navy)]">
                      {authority.code}
                    </span>
                  </td>
                  <td className="px-4 py-4 align-top font-serif text-base font-light">
                    {authority.name}
                  </td>
                  <td className="px-4 py-4 align-top text-sm font-light text-black/55">
                    {authority.description ?? "—"}
                  </td>
                  <td className="px-4 py-4 align-top">
                    {authority.roles.length > 0 ? (
                      <div className="flex flex-wrap gap-1">
                        {authority.roles.map((role) => (
                          <span
                            key={role}
                            className="rounded-sm bg-black/5 px-2 py-0.5 text-[11px] font-light text-black/60"
                          >
                            {role}
                          </span>
                        ))}
                      </div>
                    ) : (
                      <span className="text-sm font-light text-black/40">
                        Unassigned
                      </span>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  )
}
