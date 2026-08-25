import { ClientAdmin } from "@/components/portal/client-admin"

export const dynamic = "force-dynamic"

// OPPS — Client Administration. Operational stewardship/admin for canonical
// people (broad search, role/status, contact coverage, assigned agent, last
// interaction, tasks/deals/interests summary, archive). Kept OUT of CORE
// Clients, which is Lisa's daily selected-client relationship workspace.
export default async function ClientAdminPage() {
  return <ClientAdmin />
}