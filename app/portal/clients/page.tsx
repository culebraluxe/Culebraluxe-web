import { ClientAdmin } from "@/components/portal/client-admin"
import { ClientManager } from "@/components/portal/client-manager"
import { getClients } from "@/db/clients"
import { getClientAdmin } from "@/db/client-admin"

export const dynamic = "force-dynamic"

export default async function ClientsPage() {
  const [clients, adminRows] = await Promise.all([
    getClients(),
    getClientAdmin(),
  ])

  return (
    <>
      <ClientManager clients={clients} />
      <ClientAdmin rows={adminRows} />
    </>
  )
}