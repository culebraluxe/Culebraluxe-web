import { authDiagnosticsActive, devAuthLog } from '@/lib/auth/dev-auth-log'

export const dynamic = 'force-dynamic'

// AUTH-08F — custom Auth.js error page. In DEV it shows a safe stage/reason
// diagnostic; in production it shows a generic message only (never details).
const ERROR_TO_STAGE: Record<string, string> = {
  OAuthSignin: 'GOOGLE_SIGNIN',
  OAuthCallback: 'GOOGLE_CALLBACK',
  OAuthCreateAccount: 'GOOGLE_ACCOUNT',
  OAuthAccountNotLinked: 'GOOGLE_ACCOUNT_NOT_LINKED',
  Callback: 'CALLBACK',
  AccessDenied: 'ACCESS_DENIED',
  Configuration: 'CONFIGURATION',
  Default: 'UNKNOWN',
}

export default async function AuthErrorPage({
  searchParams,
}: {
  searchParams: Promise<{ error?: string }>
}) {
  const { error } = await searchParams
  const code = error ?? 'Default'
  const stage = ERROR_TO_STAGE[code] ?? 'UNKNOWN'
  const dev = authDiagnosticsActive()
  devAuthLog(stage, code)

  return (
    <main className="flex min-h-screen items-center justify-center bg-[#f5f2ec] px-6">
      <div className="w-full max-w-md rounded-sm border border-[#030f23]/10 bg-white p-8 text-center">
        <h1 className="font-serif text-xl font-light">Authentication failed</h1>
        {dev ? (
          <dl className="mt-6 space-y-2 text-sm">
            <div className="flex justify-between gap-4">
              <dt className="font-medium text-black/60">Stage</dt>
              <dd>{stage}</dd>
            </div>
            <div className="flex justify-between gap-4">
              <dt className="font-medium text-black/60">Reason</dt>
              <dd>{code}</dd>
            </div>
          </dl>
        ) : (
          <p className="mt-6 text-sm font-light text-black/50">
            Sign-in could not be completed. Please try again.
          </p>
        )}
        <a
          href="/login"
          className="mt-8 inline-flex min-h-11 items-center justify-center rounded-sm border border-[#030f23]/20 px-4 text-xs font-light uppercase tracking-[0.16em] text-[#030f23] hover:border-[#030f23]"
        >
          Back to login
        </a>
      </div>
    </main>
  )
}
