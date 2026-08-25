import { ClientManager } from "@/components/portal/client-manager"
import { ClientAdmin } from "@/components/portal/client-admin"
import { ClientsTabBar } from "@/components/portal/clients-tabbar"
import { ImportedContactsPanel } from "@/components/portal/imported-contacts"
import { getImportedContactsCount } from "@/db/imported-contacts"

export const dynamic = "force-dynamic"

export default async function ClientsPage() {
  const importedTotal = await getImportedContactsCount()

  return (
    <div data-tab-root data-active-tab="canonical">
      <style>{`
        [data-tab-root][data-active-tab="canonical"] [data-pane="imported"] { display: none; }
        [data-tab-root][data-active-tab="imported"] [data-pane="canonical"] { display: none; }
        [data-tab-root][data-active-tab="imported"] [data-pane="imported"] { display: block; }
      `}</style>

      <ClientsTabBar importedTotal={importedTotal} />

      <section data-pane="canonical">
        {/* CORE Clients experience: the restored ClientManager working pane.
            It pages the canonical `person` parent server-side (bounded 50-row
            list + independent per-person detail) and never materializes the
            whole table. ClientAdmin remains the read-only ops/admin view. */}
        <ClientManager />
        <ClientAdmin />
      </section>

      <section data-pane="imported">
        <ImportedContactsPanel initialTotal={importedTotal} />
      </section>
    </div>
  )
}