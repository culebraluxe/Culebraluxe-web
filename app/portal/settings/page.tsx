import Link from "next/link"

const sections = [
  {
    href: "/portal/settings/users",
    title: "Users",
    intro:
      "Application actors and their role assignments. CRM people are not authentication principals.",
  },
  {
    href: "/portal/settings/roles",
    title: "Roles",
    intro:
      "Named bundles of authorities, split between internal and external account types.",
  },
  {
    href: "/portal/settings/authorities",
    title: "Authorities",
    intro:
      "Coarse application capabilities that roles are built from.",
  },
]

export const dynamic = "force-dynamic"

export default async function SettingsPage() {
  return (
    <div>
      <div className="mb-8">
        <p className="text-xs font-light uppercase tracking-[0.28em] text-black/40">
          Settings
        </p>

        <h1 className="mt-3 font-serif text-4xl font-light leading-[1.1]">
          Application Security Model
        </h1>

        <p className="mt-3 max-w-3xl text-sm font-light leading-6 text-black/50">
          Canonical users, roles, and authorities. Read-only views for now —
          assignment and management arrive with authentication and authorization.
        </p>
      </div>

      <section className="grid gap-4 md:grid-cols-3">
        {sections.map((section) => (
          <Link
            key={section.href}
            href={section.href}
            className="group rounded-sm border border-[var(--portal-border)] bg-white p-6 transition hover:border-[var(--portal-navy-soft)]"
          >
            <h2 className="font-serif text-2xl font-light">{section.title}</h2>
            <p className="mt-2 text-sm font-light leading-6 text-black/50">
              {section.intro}
            </p>
            <div className="mt-4 text-xs font-light uppercase tracking-[0.16em] text-[var(--portal-navy-soft)]">
              Open →
            </div>
          </Link>
        ))}
      </section>
    </div>
  )
}
