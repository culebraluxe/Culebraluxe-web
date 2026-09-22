import { PortalYewApp } from '@/components/rust-ui/portal-yew-app'

export default async function Page({ params }: { params: Promise<Record<'formId', string>> }) {
  const { formId } = await params
  return <PortalYewApp screen="form-record" scope={formId} />
}
