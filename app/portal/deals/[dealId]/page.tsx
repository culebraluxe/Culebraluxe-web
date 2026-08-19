import { DealWorkspace } from "@/components/portal/deal-workspace"
import { getDealWorkspace } from "@/db/deal-workspace"

export const dynamic = "force-dynamic"

export default async function DealWorkspacePage({
  params,
}: {
  params: Promise<{ dealId: string }>
}) {
  const { dealId } = await params
  const workspace = await getDealWorkspace(dealId)

  return <DealWorkspace workspace={workspace} />
}
