import { Attention } from "@/components/portal/attention"
import { getAttentionSnapshot } from "@/db/attention"
import { getAttentionRelationshipContext } from "@/db/attention"
import { getActivityFeed } from "@/db/activity-feed"
import { getShowings } from "@/db/showings"

export const dynamic = "force-dynamic"

export default async function AttentionPage() {
  const [snapshot, showings, activity] = await Promise.all([
    getAttentionSnapshot(),
    getShowings(),
    getActivityFeed(50),
  ])

  // REL-INTEL — conservative relationship context for the people already in the
  // snapshot. Bounded read; returns an empty map when no evidence is linked.
  const relationshipContext = await getAttentionRelationshipContext(snapshot)

  return (
    <Attention
      snapshot={snapshot}
      showings={showings}
      activity={activity}
      relationshipContext={relationshipContext}
    />
  )
}
