import { ClientManager } from "@/components/portal/client-manager"
import { getClients } from "@/db/clients"

export const dynamic = "force-dynamic"

export default async function ClientsPage() {
  const clients = await getClients()

  return <ClientManager clients={clients} />
}