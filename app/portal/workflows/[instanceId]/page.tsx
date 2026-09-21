import { notFound } from "next/navigation"
import Link from "next/link"

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

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between">
        <span />
        <Link
          href={`/portal/runtime-inspector/${instanceId}`}
          className="text-xs font-medium uppercase tracking-[0.12em] text-[var(--portal-navy)]/60 hover:text-[var(--portal-navy)]"
        >
          Inspect Runtime →
        </Link>
      </div>
      <WorkflowInstanceDetail detail={detail} />
    </div>
  )
}
