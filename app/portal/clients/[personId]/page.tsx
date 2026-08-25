import { RelationshipDossier } from "@/components/portal/relationship-dossier"
import { getRelationshipDossier } from "@/db/dossier"
import { getRelationshipEvidenceForPerson } from "@/db/relationship-evidence"

export const dynamic = "force-dynamic"

export default async function ClientDossierPage({
  params,
}: {
  params: Promise<{ personId: string }>
}) {
  const { personId } = await params
  const dossier = await getRelationshipDossier(personId)

  // Relationship evidence is an optional enhancement: if the evidence seam is
  // not yet populated (or unavailable), the dossier renders without it.
  let relationshipEvidence: Awaited<
    ReturnType<typeof getRelationshipEvidenceForPerson>
  > = []
  try {
    relationshipEvidence = await getRelationshipEvidenceForPerson(personId)
  } catch {
    relationshipEvidence = []
  }

  return (
    <RelationshipDossier
      dossier={dossier}
      relationshipEvidence={relationshipEvidence}
    />
  )
}
