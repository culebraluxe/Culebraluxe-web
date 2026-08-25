import { ClientManager } from "@/components/portal/client-manager"

export const dynamic = "force-dynamic"

// CORE Clients — the canonical relationship working surface. It begins directly
// with the People rail and the selected Client workspace: Command + Status,
// Client Card + Contact History, Interests + Notes. Staging/imported and
// Client Administration (operational stewardship) are intentionally NOT here —
// Imported Contacts stewardship lives on OPPS Identity Quality, and Client
// Administration lives on its own OPPS route.
export default async function ClientsPage() {
  return <ClientManager />
}
