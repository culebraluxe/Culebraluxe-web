import { redirect } from 'next/navigation'

import { RustUi } from '@/components/rust-ui/rust-ui'
import { createAuthJsSessionAdapter } from '@/lib/auth/authjs-session-adapter'
import { resolvePortalAccess } from '@/lib/auth/require-portal-access'

export const dynamic = 'force-dynamic'

// Flight Recorder application ownership is Yew/MVI. The mature React console remains a bounded renderer for
// virtualization, SVG graphs, swimlanes and event inspection; it performs no network request of its own.
export default async function Page({
  params,
}: {
  params: Promise<Record<'instanceId', string>>
}) {
  const access = await resolvePortalAccess(
    createAuthJsSessionAdapter(),
    'tech.access',
  )
  if (!access.ok) redirect(access.redirectTo)

  const { instanceId } = await params
  return <RustUi screen="trace-record" scope={instanceId} />
}
