import { PortalYewApp } from '@/components/rust-ui/portal-yew-app'

export default async function Page({ params }: { params: Promise<Record<'dealId', string>> }) {
  const { dealId } = await params
  return <PortalYewApp screen="deal-record" scope={dealId} />
}
