import { redirect } from 'next/navigation'

import { RustUi } from '@/components/rust-ui/rust-ui'
import { createAuthJsSessionAdapter } from '@/lib/auth/authjs-session-adapter'
import { resolvePortalAccess } from '@/lib/auth/require-portal-access'

export const dynamic = 'force-dynamic'

// TECH is an operator surface. Yew owns the screen; the SVAR sorter is a rendering-only React island.
// Pass 1 deliberately preserves the mature read/write seams while the control-plane commands move to Rust.
export default async function Page() {
  const access = await resolvePortalAccess(
    createAuthJsSessionAdapter(),
    'tech.access',
  )
  if (!access.ok) redirect(access.redirectTo)

  return <RustUi screen="tech" />
}
