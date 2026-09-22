import { PortalYewApp } from '@/components/rust-ui/portal-yew-app'

export default async function Page({ params }: { params: Promise<Record<'instanceId', string>> }) {
  const { instanceId } = await params
  return <PortalYewApp screen="workflow-record" scope={instanceId} />
}
