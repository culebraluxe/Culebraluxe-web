import { redirect } from 'next/navigation'

import { PortalYewApp } from '@/components/rust-ui/portal-yew-app'
import { createAuthJsSessionAdapter } from '@/lib/auth/authjs-session-adapter'
import { resolvePortalAccess } from '@/lib/auth/require-portal-access'

export const dynamic = 'force-dynamic'

export default async function Page() {
  const access = await resolvePortalAccess(
    createAuthJsSessionAdapter(),
    'tech.access',
  )
  if (!access.ok) redirect(access.redirectTo)

  return <PortalYewApp screen="tech-flight-recorder" />
}
