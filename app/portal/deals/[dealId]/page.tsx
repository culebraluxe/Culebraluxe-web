import { RustUi } from '@/components/rust-ui/rust-ui'

export default async function Page({ params }: { params: Promise<Record<'dealId', string>> }) {
  const { dealId } = await params
  return <RustUi screen="deal-record" scope={dealId} />
}
