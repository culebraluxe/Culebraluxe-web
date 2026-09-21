import { Reporting } from "@/components/portal/reporting"
import { getReportingSnapshot } from "@/db/reporting"

export const dynamic = "force-dynamic"

export default async function ReportingPage() {
  const snapshot = await getReportingSnapshot()

  return <Reporting snapshot={snapshot} />
}
