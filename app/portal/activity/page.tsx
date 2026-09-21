import { ActivityFeed } from "@/components/portal/activity-feed"
import { getActivityFeed } from "@/db/activity-feed"

export const dynamic = "force-dynamic"

export default async function ActivityPage() {
  const entries = await getActivityFeed(50)

  return <ActivityFeed entries={entries} />
}
