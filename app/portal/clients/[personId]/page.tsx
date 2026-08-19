import { RelationshipDossier } from "@/components/portal/relationship-dossier"
import { getRelationshipDossier } from "@/db/dossier"

export const dynamic = "force-dynamic"

export default async function ClientDossierPage({
  params,
}: {
  params: Promise<{ personId: string }>
}) {
  const { personId } = await params
  const dossier = await getRelationshipDossier(personId)

  return <RelationshipDossier dossier={dossier} />
}
