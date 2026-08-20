import { Showings } from "@/components/portal/showings"
import { getShowings } from "@/db/showings"

export const dynamic = "force-dynamic"

export default async function ShowingsPage() {
  const showings = await getShowings()

  return <Showings showings={showings} />
}
