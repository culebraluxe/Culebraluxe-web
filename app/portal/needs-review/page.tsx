import { NeedsReview } from "@/components/portal/needs-review"
import { getNeedsReviewItems } from "@/db/needs-review"

export const dynamic = "force-dynamic"

export default async function NeedsReviewPage() {
  const items = await getNeedsReviewItems()

  return <NeedsReview items={items} />
}
