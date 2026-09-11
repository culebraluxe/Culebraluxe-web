import { IdentityQuality } from "@/components/portal/identity-quality"
import { getIdentityQuality } from "@/db/identity-quality"

export const dynamic = "force-dynamic"

export default async function IdentityQualityPage() {
  const snapshot = await getIdentityQuality()

  return <IdentityQuality snapshot={snapshot} />
}
