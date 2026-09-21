import { Attention } from "@/components/portal/attention"
import { getAttentionSnapshot, getAttentionRelationshipContext, getContactEvidence } from "@/db/attention"
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

  // CORE-DAILY-05 — native contact evidence (canonical person_identity) for the
  // people in the follow-up queue, so each actionable item can launch contact.
  const personIds = Array.from(
    new Set(
      [...snapshot.overdueTasks, ...snapshot.dueSoonTasks]
        .map((t) => t.personId)
        .filter((id): id is string => Boolean(id)),
    ),
  )
  const contactEvidence = await getContactEvidence(personIds)

  return (
    <Attention
      snapshot={snapshot}
      showings={showings}
      activity={activity}
      relationshipContext={relationshipContext}
      contactEvidence={contactEvidence}
    />
  )
}
