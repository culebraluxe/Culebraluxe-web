import { ClientManager } from "@/components/portal/client-manager"
import { ClientAdmin } from "@/components/portal/client-admin"

export const dynamic = "force-dynamic"

// CORE Clients — the canonical CRM working surface. It begins directly with
// the People rail and the selected Client workspace. The staging/imported
// strip (Canonical Clients | Imported Contacts) is intentionally NOT here;
// Imported Contacts stewardship lives on the OPPS Identity Quality surface.
export default async function ClientsPage() {
  return (
    <>
      <ClientManager />
      <ClientAdmin />
    </>
  )
}
