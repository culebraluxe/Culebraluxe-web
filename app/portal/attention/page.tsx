import { Attention } from "@/components/portal/attention"
import { getAttentionSnapshot } from "@/db/attention"

export const dynamic = "force-dynamic"

export default async function AttentionPage() {
  const snapshot = await getAttentionSnapshot()

  return <Attention snapshot={snapshot} />
}
