import { ClientLens } from '@/components/portal/client-lens'

export const dynamic = 'force-dynamic'

/**
 * Side-by-side architecture proving ground for the Clients/CRM surface.
 * The production /portal/clients page remains untouched.
 */
export default function ClientLensPage() {
  return <ClientLens />
}
