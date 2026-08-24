import { ClientAdmin } from "@/components/portal/client-admin"
import { ClientManager } from "@/components/portal/client-manager"
import { ClientsTabBar } from "@/components/portal/clients-tabbar"
import { ImportedContactsPanel } from "@/components/portal/imported-contacts"
import { getClients } from "@/db/clients"
import { getClientAdmin } from "@/db/client-admin"
import { getImportedContactsCount } from "@/db/imported-contacts"
import { listAssignableAgents } from "@/db/person-admin"

export const dynamic = "force-dynamic"

export default async function ClientsPage() {
  const [clients, adminRows, agents, importedTotal] = await Promise.all([
    getClients(),
    getClientAdmin(),
    listAssignableAgents(),
    getImportedContactsCount(),
  ])

  return (
    <div data-tab-root data-active-tab="canonical">
      <style>{`
        [data-tab-root][data-active-tab="canonical"] [data-pane="imported"] { display: none; }
        [data-tab-root][data-active-tab="imported"] [data-pane="canonical"] { display: none; }
        [data-tab-root][data-active-tab="imported"] [data-pane="imported"] { display: block; }
      `}</style>

      <ClientsTabBar importedTotal={importedTotal} />

      <section data-pane="canonical">
        <ClientManager clients={clients} agents={agents} />
        <ClientAdmin rows={adminRows} />
      </section>

      <section data-pane="imported">
        <ImportedContactsPanel initialTotal={importedTotal} />
      </section>
    </div>
  )
}