import { Attention } from "@/components/portal/attention"
import { getAttentionSnapshot } from "@/db/attention"
import { getActivityFeed } from "@/db/activity-feed"
import { getShowings } from "@/db/showings"

export const dynamic = "force-dynamic"

export default async function AttentionPage() {
  const [snapshot, showings, activity] = await Promise.all([
    getAttentionSnapshot(),
    getShowings(),
    getActivityFeed(50),
  ])

  return <Attention snapshot={snapshot} showings={showings} activity={activity} />
}
