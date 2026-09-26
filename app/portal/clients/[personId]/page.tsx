import { RustUi } from '@/components/rust-ui/rust-ui'

export default async function Page({ params }: { params: Promise<Record<'personId', string>> }) {
  const { personId } = await params
  return <RustUi screen="client-record" scope={personId} />
}
