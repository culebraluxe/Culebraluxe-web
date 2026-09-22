import { PortalYewApp } from '@/components/rust-ui/portal-yew-app'

export default async function Page({ params }: { params: Promise<Record<'personId', string>> }) {
  const { personId } = await params
  return <PortalYewApp screen="client-record" scope={personId} />
}
