import { notFound } from 'next/navigation'

import { PnsLens } from '@/components/portal/pns-lens'
import {
  getActiveTemplate,
  PURCHASE_SALE_TEMPLATE_ID,
} from '@/lib/forms/template-registry'

export const dynamic = 'force-dynamic'

/**
 * Architecture proving ground for PR-PNS ownership and relationship roles.
 * Production /portal/forms, Deal, issued documents, and persisted form workflow
 * remain untouched.
 */
export default function PnsLensPage() {
  const template = getActiveTemplate(PURCHASE_SALE_TEMPLATE_ID)
  if (!template) notFound()
  return <PnsLens template={template} />
}
