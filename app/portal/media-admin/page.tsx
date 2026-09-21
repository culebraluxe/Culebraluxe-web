import { MediaAdmin } from "@/components/portal/media-admin"
import { getMediaAdmin } from "@/db/media-admin"

export const dynamic = "force-dynamic"

export default async function MediaAdminPage() {
  const snapshot = await getMediaAdmin()

  return <MediaAdmin snapshot={snapshot} />
}
