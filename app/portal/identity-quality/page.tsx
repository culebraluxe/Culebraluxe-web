import { IdentityQuality } from "@/components/portal/identity-quality"
import { ImportedContactsPanel } from "@/components/portal/imported-contacts"
import { getIdentityQuality } from "@/db/identity-quality"
import { getImportedContactsCount } from "@/db/imported-contacts"

export const dynamic = "force-dynamic"

export default async function IdentityQualityPage() {
  const snapshot = await getIdentityQuality()
  const importedTotal = await getImportedContactsCount()

  return (
    <>
      <IdentityQuality snapshot={snapshot} />
      {/* Imported Contacts stewardship lives on this OPPS surface (not on the
          CORE Clients screen). Relational load projection, never canonical. */}
      <div className="mt-10">
        <ImportedContactsPanel initialTotal={importedTotal} />
      </div>
    </>
  )
}
