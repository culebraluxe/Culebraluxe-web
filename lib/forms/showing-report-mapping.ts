import type { FormInstance } from '@/db/document-form-instance'
import type { SaveShowingReportRequest, ShowingReportOutcome } from '@/services/showing'
import { compactFormValue, nullableFormValue } from './service-binding-core'

const OUTCOMES = new Set<ShowingReportOutcome>([
  'Interested',
  'Second showing',
  'Offer expected',
  'Not a fit',
])

export type ShowingReportDraft = Pick<
  FormInstance,
  'id' | 'personId' | 'propertyId' | 'fieldValues' | 'sections'
>

export function mapShowingReportDraft(
  form: ShowingReportDraft,
  showingId: string,
): SaveShowingReportRequest {
  if (!form.personId || !form.propertyId) {
    throw new Error('SHOW-RPT requires explicit Person and Property context.')
  }

  const date = nullableFormValue(form.fieldValues.showingDate)
  if (date && !/^\d{4}-\d{2}-\d{2}$/.test(date)) {
    throw new Error('Showing date must use YYYY-MM-DD.')
  }

  const rawOutcome = compactFormValue(form.fieldValues.outcome)
  if (rawOutcome && !OUTCOMES.has(rawOutcome as ShowingReportOutcome)) {
    throw new Error(`Unsupported Showing Report outcome: ${rawOutcome}`)
  }

  const rawScore = compactFormValue(form.fieldValues.feedbackScore)
  let interestScore: number | null = null
  if (rawScore) {
    interestScore = Number(rawScore)
    if (!Number.isInteger(interestScore) || interestScore < 1 || interestScore > 5) {
      throw new Error('Showing interest score must be an integer from 1 to 5.')
    }
  }

  return {
    showingId,
    personId: form.personId,
    propertyId: form.propertyId,
    showingDate: date,
    duration: nullableFormValue(form.fieldValues.duration),
    outcome: rawOutcome ? (rawOutcome as ShowingReportOutcome) : null,
    interestScore,
    feedback: nullableFormValue(form.sections.feedback),
    followUp: nullableFormValue(form.sections.followUp),
  }
}
