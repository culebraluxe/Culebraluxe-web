import Link from 'next/link'
import { redirect } from 'next/navigation'

import { auth } from '@/auth'
import { devAuthLog } from '@/lib/auth/dev-auth-log'

export const dynamic = 'force-dynamic'

// AUTH-08F — TEMPORARY DEV proof page. Renders ONLY safe session diagnostics
// (authentication success, provider, subject/session presence). Never shows
// tokens, cookies, secrets, codes, or raw claims. In production it redirects to
// the Portal (this page is for the DEV baseline proof only).
export default async function PortalAuthProofPage() {
  if (process.env.NODE_ENV === 'production') redirect('/portal')

  const session = await auth()
  const sub = session?.user?.sub ?? null
  const provider = session?.user?.provider ?? null

  devAuthLog(
    sub ? 'AUTH_SESSION_CREATED' : 'AUTH_SESSION_MISSING',
    sub ? undefined : 'NO_SESSION',
  )

  // AUTH-08G — DEV provisioning aid. When an authenticated Google session is
  // presented, record the STABLE provider subject server-side so the operator
  // can create the explicit auth_identity -> app_user mapping for this account.
  // This page already redirects to /portal in production, so this only ever runs
  // in DEV. The subject is a stable, non-secret identifier (never a token,
  // cookie, or secret). Distinct prefix so it never collides with the
  // secret-free auth-flight stream.
  if (sub) {
    console.log(`[dev-identity-capture] provider=${provider ?? 'unknown'} google_sub=${sub}`)
  }

  return (
    <main className="flex min-h-screen items-center justify-center bg-[#f5f2ec] px-6">
      <div className="w-full max-w-md rounded-sm border border-[#030f23]/10 bg-white p-8">
        <h1 className="font-serif text-xl font-light">Portal Auth Proof</h1>
        <p className="mt-2 text-sm font-light leading-6 text-black/50">
          Safe session diagnostics only.
        </p>
        <dl className="mt-6 space-y-2 text-sm">
          <div className="flex justify-between gap-4">
            <dt className="font-medium text-black/60">Authentication</dt>
            <dd className={sub ? 'text-green-700' : 'text-red-700'}>
              {sub ? 'SUCCESS' : 'FAILED'}
            </dd>
          </div>
          <div className="flex justify-between gap-4">
            <dt className="font-medium text-black/60">Provider</dt>
            <dd>{provider ?? 'none'}</dd>
          </div>
          <div className="flex justify-between gap-4">
            <dt className="font-medium text-black/60">Subject present</dt>
            <dd>{sub ? 'YES' : 'NO'}</dd>
          </div>
          <div className="flex justify-between gap-4">
            <dt className="font-medium text-black/60">Session present</dt>
            <dd>{session ? 'YES' : 'NO'}</dd>
          </div>
          {sub ? (
            <div className="mt-2 border-t border-black/10 pt-2">
              <dt className="font-medium text-black/60">
                Google subject (DEV provisioning)
              </dt>
              <dd className="mt-1 break-all font-mono text-xs text-black/60">
                {sub}
              </dd>
            </div>
          ) : null}
        </dl>
        <div className="mt-8 flex flex-wrap gap-4">
          <Link
            href="/login"
            className="inline-flex min-h-11 items-center rounded-sm border border-[#030f23]/20 px-4 text-xs font-light uppercase tracking-[0.16em] text-[#030f23] hover:border-[#030f23]"
          >
            Back to login
          </Link>
          <Link
            href="/portal"
            className="inline-flex min-h-11 items-center rounded-sm bg-[#030f23] px-4 text-xs font-light uppercase tracking-[0.16em] text-white"
          >
            Go to Portal
          </Link>
        </div>
      </div>
    </main>
  )
}
