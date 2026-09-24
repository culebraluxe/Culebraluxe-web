import { RustUiHost } from '@/components/rust-ui/host'
import { signIn } from '@/auth'
import { devAuthLog } from '@/lib/auth/dev-auth-log'

// ---------------------------------------------------------------------------
// CONVERTED TO RUST (screen: login).
//
// What this route rendered now lives in rust/ui/src/view.rs, fed by the rows route. The route itself is unchanged,
// which is what keeps every link and bookmark working.
// ---------------------------------------------------------------------------

export default function Page() {
  return (
    <div className="min-h-screen bg-background">
      <form action={async () => {
        'use server'
        devAuthLog('AUTH_SIGNIN_STARTED')
        await signIn('google', { redirectTo: '/portal-auth-proof' })
      }}>
        <RustUiHost rowsPath="/api/rust-ui/public-rows" start="login" />
      </form>
    </div>
  )
}
