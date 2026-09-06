import { notFound } from 'next/navigation'

import { FormLens } from '@/components/portal/form-lens'
import {
  getActiveTemplate,
  LISTING_AGREEMENT_TEMPLATE_ID,
} from '@/lib/forms/template-registry'

export const dynamic = 'force-dynamic'

/**
 * Side-by-side architecture proving ground for Forms composition.
 * The production /portal/forms surface and persisted form workflow remain untouched.
 */
export default function FormLensPage() {
  const template = getActiveTemplate(LISTING_AGREEMENT_TEMPLATE_ID)
  if (!template) notFound()
  return <FormLens template={template} />
}
