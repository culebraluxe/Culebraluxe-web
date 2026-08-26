import type { ReactNode } from "react"
import { redirect } from "next/navigation"

import { createAuthJsSessionAdapter } from "@/lib/auth/authjs-session-adapter"
import { resolvePortalAccess } from "@/lib/auth/require-portal-access"

export const dynamic = "force-dynamic"

// AUTH-09E — TECH operating-world landing. Requires tech.access (ROOT only).
// The /portal layout already requires portal.read for the whole portal; this
// page adds the TECH-only authority. Non-tech actors (BUSINESS_POWER) are
// redirected to /login/unauthorized even on a direct URL. Nav hiding is cosmetic;
// this server-side guard is the security boundary.
export default async function TechPage() {
  const result = await resolvePortalAccess(
    createAuthJsSessionAdapter(),
    "tech.access",
  )
  if (!result.ok) redirect(result.redirectTo)

  return (
    <div className="max-w-3xl">
      <h1 className="font-serif text-2xl font-light text-[var(--portal-ink)]">
        TECH Overview
      </h1>
      <p className="mt-2 text-sm font-light leading-6 text-[var(--portal-ink)]/60">
        Engineering and platform capability. This surface requires the{" "}
        <code>tech.access</code> authority and is restricted to ROOT actors.
      </p>
      <div className="mt-6 rounded-sm border border-black/10 bg-white p-6 text-sm font-light text-[var(--portal-ink)]/70">
        <dl className="space-y-2">
          <div className="flex justify-between">
            <dt className="text-black/60">Actor</dt>
            <dd>{result.actor.displayName}</dd>
          </div>
          <div className="flex justify-between">
            <dt className="text-black/60">Role</dt>
            <dd>{result.actor.roleCodes.join(", ") || "—"}</dd>
          </div>
          <div className="flex justify-between">
            <dt className="text-black/60">tech.access</dt>
            <dd>granted</dd>
          </div>
        </dl>
      </div>
    </div>
  )
}
