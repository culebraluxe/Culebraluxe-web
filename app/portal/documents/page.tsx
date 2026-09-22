import { redirect } from 'next/navigation'

import { PortalYewApp } from '@/components/rust-ui/portal-yew-app'
import { createAuthJsSessionAdapter } from '@/lib/auth/authjs-session-adapter'
import { resolvePortalAccess } from '@/lib/auth/require-portal-access'

export const dynamic = 'force-dynamic'

// Cabinet keeps the mature deal.read route guard. Yew owns rendering and local
// filtering; Rust Vault owns the repository read and actor scoping.
export default async function Page() {
  const access = await resolvePortalAccess(
    createAuthJsSessionAdapter(),
    'deal.read',
  )
  if (!access.ok) redirect(access.redirectTo)

  return <PortalYewApp screen="cabinet" />
}
