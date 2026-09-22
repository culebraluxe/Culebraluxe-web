import { PortalYewApp } from '@/components/rust-ui/portal-yew-app'

export const metadata = {
  title: 'Seller Strategy',
  robots: { index: false, follow: false },
}

export default function SellerStrategyPage() {
  return <PortalYewApp screen="seller-strategy" />
}
