import { ClientsDirectory } from "@/components/portal/clients-directory"
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
        {/* Both panes are independent client components that page the canonical
            `person` parent server-side (50/page). The canonical pane renders
            immediately and never blocks on imported rows. */}
        <ClientsDirectory />
        <ClientAdmin />
      </section>

      <section data-pane="imported">
        <ImportedContactsPanel initialTotal={importedTotal} />
      </section>
    </div>
  )
}