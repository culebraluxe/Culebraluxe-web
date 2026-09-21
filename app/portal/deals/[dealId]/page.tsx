import { DealWorkspace } from "@/components/portal/deal-workspace"
import { getDealWorkspace } from "@/legacy/db/deal-workspace"
import { getSettingsUsers } from "@/legacy/db/settings-auth"

export const dynamic = "force-dynamic"

export default async function DealWorkspacePage({
  params,
}: {
  params: Promise<{ dealId: string }>
}) {
  const { dealId } = await params
  const [workspace, users] = await Promise.all([
    getDealWorkspace(dealId),
    getSettingsUsers(),
  ])

  return (
    <DealWorkspace
      workspace={workspace}
      ownerCandidates={users
        .filter((user) => user.active)
        .map((user) => ({
          id: user.id,
          displayName: user.displayName,
          email: user.email,
        }))}
    />
  )
}
