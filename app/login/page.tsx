import Link from "next/link"
import { signIn } from "@/auth"

export const dynamic = "force-dynamic"

// AUTH-BOUNDARY — the login route is the portal entry. If the Auth.js secret is
// not configured, the portal cannot authenticate anyone, so we show a controlled
// "Portal temporarily unavailable" state instead of a sign-in form that cannot
// work. Server-side check only — no auth instance, no secret value surfaced.
const AUTH_CONFIGURED = Boolean(
  process.env.AUTH_SECRET || process.env.NEXTAUTH_SECRET,
)

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

        {AUTH_CONFIGURED ? (
          <div className="mt-10 rounded-sm border border-[#030f23]/10 bg-white p-8">
            <h1 className="font-serif text-xl font-light">Sign in</h1>
            <p className="mt-2 text-sm font-light leading-6 text-black/50">
              Access is for the CulebraLuxe team. There is no public sign-up —
              accounts are provisioned by an administrator.
            </p>

            <form
              action={async () => {
                "use server"
                await signIn("google", { redirectTo: "/portal" })
              }}
              className="mt-8 w-full"
            >
              <button
                type="submit"
                className="flex min-h-12 w-full items-center justify-center gap-3 rounded-sm border border-[#030f23]/15 px-4 text-sm font-light text-[#030f23] transition hover:border-[#030f23]"
              >
                Continue with Google
              </button>
            </form>

            <p className="mt-4 text-xs font-light text-black/40">
              Having trouble? Contact a CulebraLuxe administrator.
            </p>
          </div>
        ) : (
          <div className="mt-10 rounded-sm border border-[#030f23]/10 bg-white p-8">
            <h1 className="font-serif text-xl font-light">
              Portal temporarily unavailable
            </h1>
            <p className="mt-2 text-sm font-light leading-6 text-black/50">
              The Portal could not be reached right now. This does not affect the
              public CulebraLuxe website. Please contact a CulebraLuxe
              administrator.
            </p>
            <div className="mt-8">
              <Link
                href="/"
                className="flex min-h-12 w-full items-center justify-center rounded-sm border border-[#030f23]/15 px-4 text-sm font-light text-[#030f23] transition hover:border-[#030f23]"
              >
                Return home
              </Link>
            </div>
          </div>
        )}

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
