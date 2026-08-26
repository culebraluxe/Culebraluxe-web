import { IssuesQueue } from "@/components/portal/issues-queue"
import { getIssueQueue } from "@/db/issues"

export const dynamic = "force-dynamic"

// OPS-11A — Operational Issue Queue + Runbook dashboard (OPPS surface).
// Server-fetches the first bounded page of OPEN operations issues; the client
// renders the two-pane queue and pages via /api/portal/issues.
export default async function IssuesPage() {
  const page = await getIssueQueue({
    scope: "OPERATIONS_EXCEPTION",
    state: "OPEN",
    page: 1,
    pageSize: 50,
  })

  return <IssuesQueue initialPage={page} />
}
