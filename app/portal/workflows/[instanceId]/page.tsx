import { notFound } from "next/navigation"

import { WorkflowInstanceDetail } from "@/components/portal/workflow-instance-detail"
import { getWorkflowDetail } from "@/workflow_app/read-service"

export const dynamic = "force-dynamic"

export default async function WorkflowInstancePage({
  params,
}: {
  params: Promise<{ instanceId: string }>
}) {
  const { instanceId } = await params
  const detail = await getWorkflowDetail(instanceId)
  if (!detail) notFound()

  return <WorkflowInstanceDetail detail={detail} />
}
