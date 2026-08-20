import Link from "next/link"

export const dynamic = "force-dynamic"

export default function LoginUnauthorizedPage() {
  return (
    <main className="flex min-h-screen items-center justify-center bg-[#f5f2ec] px-6">
      <div className="w-full max-w-md rounded-sm border border-[#030f23]/10 bg-white p-8">
        <h1 className="font-serif text-2xl font-light">Access not authorized</h1>
        <p className="mt-3 text-sm font-light leading-6 text-black/60">
          This account is authenticated but is not authorized for CulebraLuxe.
          Accounts are provisioned by an administrator — there is no self-service
          sign-up or automatic access.
        </p>
        <p className="mt-4 text-sm font-light leading-6 text-black/50">
          If you believe this is a mistake, contact a CulebraLuxe administrator
          and provide the identity shown by your sign-in provider. Your password,
          secret, and provider credentials are never shared or displayed here.
        </p>
        <Link
          href="/login"
          className="mt-8 inline-flex min-h-11 items-center justify-center rounded-sm bg-[#030f23] px-5 text-xs font-light uppercase tracking-[0.16em] text-white transition hover:bg-[#0b2240]"
        >
          Back to sign in
        </Link>
      </div>
    </main>
  )
}
