import { randomUUID } from 'node:crypto'

import { bindFormInstanceToShowing, getFormShowingId } from '@/db/form-service-lineage'
import { SHOWING_OPERATIONS } from '@/services/showing'
import { formShowingService } from './form-service-runtime'
import { formServiceContext, serviceValue } from './service-binding-core'
import { mapShowingReportDraft, type ShowingReportDraft } from './showing-report-mapping'

export async function syncShowingReportForm(
  form: ShowingReportDraft,
  actorId: string | null,
): Promise<string> {
  const showingId = (await getFormShowingId(form.id)) ?? randomUUID()
  const request = mapShowingReportDraft(form, showingId)
  const saved = await serviceValue(
    formShowingService.execute({
      operation: SHOWING_OPERATIONS.SAVE_REPORT,
      payload: request,
      context: formServiceContext(actorId),
    }),
    'Showing Report service binding failed',
  )
  await bindFormInstanceToShowing({ formInstanceId: form.id, showingId: saved.id })
  return saved.id
}
