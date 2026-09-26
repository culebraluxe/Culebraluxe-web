import { RustUi } from '@/components/rust-ui/rust-ui'

export default async function Page({ params }: { params: Promise<Record<'instanceId', string>> }) {
  const { instanceId } = await params
  return <RustUi screen="workflow-record" scope={instanceId} />
}
