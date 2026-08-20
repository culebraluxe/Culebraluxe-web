import Link from "next/link"

export const dynamic = "force-dynamic"

export default function LoginPage() {
  return (
    <main className="flex min-h-screen items-center justify-center bg-[#f5f2ec] px-6">
      <div className="w-full max-w-sm">
        <div className="text-center">
          <div className="font-serif text-2xl font-light uppercase tracking-[0.08em] text-[#030f23]">
            CulebraLuxe
          </div>
          <div className="mt-1 text-[10px] font-light uppercase tracking-[0.32em] text-[#030f23]/50">
            Private Portal
          </div>
        </div>

        <div className="mt-10 rounded-sm border border-[#030f23]/10 bg-white p-8">
          <h1 className="font-serif text-xl font-light">Sign in</h1>
          <p className="mt-2 text-sm font-light leading-6 text-black/50">
            Access is for the CulebraLuxe team. There is no public sign-up —
            accounts are provisioned by an administrator.
          </p>

          <Link
            href="/api/auth/signin/google"
            className="mt-8 flex min-h-12 w-full items-center justify-center gap-3 rounded-sm border border-[#030f23]/15 px-4 text-sm font-light text-[#030f23] transition hover:border-[#030f23]"
          >
            Continue with Google
          </Link>

          <p className="mt-4 text-xs font-light text-black/40">
            Having trouble? Contact a CulebraLuxe administrator.
          </p>
        </div>

        <div className="mt-6 text-center">
          <Link
            href="/login/recovery"
            className="text-xs font-light text-[#030f23]/45 underline-offset-2 hover:underline"
          >
            Emergency administrative access
          </Link>
        </div>
      </div>
    </main>
  )
}
