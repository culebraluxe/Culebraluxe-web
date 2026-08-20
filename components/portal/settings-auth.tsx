import type {
  SettingsAuthority,
  SettingsRole,
  SettingsUser,
} from "@/db/settings-auth"

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
          : "bg-[#e5e2ec] text-[#5d5870]"
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
                      <span className="text-[#40584b]">Active</span>
                    ) : (
                      <span className="text-[#8a4b2a]">Inactive</span>
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
