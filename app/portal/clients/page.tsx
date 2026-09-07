import { ClientLens } from "@/components/portal/client-lens"

export const dynamic = "force-dynamic"

// CORE Clients — canonical relationship working surface (new MVI ClientLens).
// Legacy ClientManager + the /portal/client-lens sidecar route remain as an
// emergency reference; rollback is one git revert of this file.
export default function ClientsPage() {
  return <ClientLens />
}
