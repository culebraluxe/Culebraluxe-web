import { formContentFingerprint } from '@/lib/forms/artifact-identity'
import { documentBodyText, resolveDocumentBody } from '@/lib/forms/format'
import { LISTING_CANONICAL_FIELD_NAMES } from '@/lib/forms/listing-field-binding'
import { applyDateDefaults } from '@/lib/forms/offer-letter-data'
import type { TemplateDefinition } from '@/lib/forms/template-types'
import {
  BasePageController,
  type PageOperationContext,
  type PageOperationDefinitions,
} from '../runtime'
import {
  isFormEditorDirty,
  type FormEditorIntentMap,
  type FormEditorIssuedDocument,
  type FormEditorPageModel,
  type FormEditorSignatureState,
} from './model'
import type { FormEditorSource } from './source'

function initialDetailsText(
  template: TemplateDefinition,
  sections: Record<string, string>,
  values: Record<string, string>,
) {
  return resolveDocumentBody(template, values, sections)
}

function cloneRecord(values: Record<string, string>) {
  return { ...values }
}

export class FormEditorController extends BasePageController<
  FormEditorPageModel,
  FormEditorIntentMap
> {
  protected readonly operations: PageOperationDefinitions<
    FormEditorPageModel,
    FormEditorIntentMap
  >

  constructor(
    private readonly source: FormEditorSource,
    private readonly formId: string,
    private readonly template: TemplateDefinition,
    initial: {
      fieldValues: Record<string, string>
      sections: Record<string, string>
      issuedDocument?: FormEditorIssuedDocument | null
      signatureState?: FormEditorSignatureState | null
    },
  ) {
    const detailsText = initialDetailsText(
      template,
      initial.sections,
      initial.fieldValues,
    )

    super({
      values: applyDateDefaults(template, cloneRecord(initial.fieldValues)),
      sections: cloneRecord(initial.sections),
      detailsText,
      bodyEdited: initial.sections.bodyEdited === 'true',
      saved: {
        values: cloneRecord(initial.fieldValues),
        sections: cloneRecord(initial.sections),
        detailsText,
      },
      message: null,
      error: null,
      issued: initial.issuedDocument ?? null,
      signatureState: initial.signatureState ?? null,
      busy: false,
      draftSaving: false,
    })

    this.operations = {
      'formEditor.fieldChanged': {
        description: 'Apply one form-field intent and keep generated document text aligned until manually edited.',
        execution: 'parallel',
        handle: ({ name, value }, context) => {
          context.update((model) => {
            const values = { ...model.values, [name]: value }
            return {
              ...model,
              values,
              detailsText: model.bodyEdited
                ? model.detailsText
                : documentBodyText(this.template, values, model.sections),
              message: null,
              error: null,
            }
          })
        },
      },
      'formEditor.sectionChanged': {
        description: 'Apply one editable XML-section intent and keep the generated document body aligned.',
        execution: 'parallel',
        handle: ({ name, value }, context) => {
          context.update((model) => {
            const sections = { ...model.sections, [name]: value }
            return {
              ...model,
              sections,
              detailsText: model.bodyEdited
                ? model.detailsText
                : documentBodyText(this.template, model.values, sections),
              message: null,
              error: null,
            }
          })
        },
      },
      'formEditor.detailsChanged': {
        description: 'Record a deliberate edit to the Word-like document body.',
        execution: 'parallel',
        handle: ({ value }, context) => {
          context.update((model) => ({
            ...model,
            detailsText: value,
            bodyEdited: true,
            message: null,
            error: null,
          }))
        },
      },
      'formEditor.refreshListing': {
        description: 'Re-read canonical Person/Property facts and fill only blank Listing fields.',
        execution: 'latest',
        handle: async ({ personId }, context) => {
          const result = await this.source.refreshListing(personId)
          if (!result.ok) {
            context.update((model) => ({ ...model, error: result.message }))
            return 0
          }

          const before = context.snapshot()
          const values = { ...before.values }
          let filled = 0
          for (const name of LISTING_CANONICAL_FIELD_NAMES) {
            const origin = result.data.origins[name]
            const value = result.data.fields[name]?.trim() ?? ''
            if (origin !== 'person' && origin !== 'property') continue
            if (!value || (values[name] ?? '').trim()) continue
            values[name] = value
            filled += 1
          }

          context.update((model) => ({
            ...model,
            values,
            detailsText: model.bodyEdited
              ? model.detailsText
              : documentBodyText(this.template, values, model.sections),
            message:
              filled > 0
                ? `Refreshed client data · ${filled} field${filled === 1 ? '' : 's'} filled`
                : 'Client data already current',
            error: null,
          }))
          return filled
        },
      },
      'formEditor.saveDraft': {
        description: 'Persist the current mutable form draft through the Forms action/service-binding seam.',
        execution: 'serial',
        handle: async ({ quiet }, context) => this.persistDraft(context, Boolean(quiet)),
      },
      'formEditor.discard': {
        description: 'Discard local edits and restore the last persisted draft snapshot.',
        execution: 'parallel',
        handle: (_request, context) => {
          context.update((model) => ({
            ...model,
            values: cloneRecord(model.saved.values),
            sections: cloneRecord(model.saved.sections),
            detailsText: model.saved.detailsText,
            bodyEdited: model.saved.sections.bodyEdited === 'true',
            message: 'Changes discarded',
            error: null,
          }))
        },
      },
      'formEditor.create': {
        description: 'Create another form through the canonical Forms action seam and return its id to navigation.',
        execution: 'serial',
        handle: async (request, context) => {
          context.update((model) => ({ ...model, busy: true, error: null }))
          try {
            const result = await this.source.create(request)
            if (!result.ok) {
              context.update((model) => ({ ...model, busy: false, error: result.message }))
              return null
            }
            return result.data.formId
          } catch (error) {
            const message = error instanceof Error ? error.message : 'Could not start a new form.'
            context.update((model) => ({ ...model, busy: false, error: message }))
            return null
          }
        },
      },
      'formEditor.grokFill': {
        description: 'Apply Grok form-fill output to the same PageModel used by direct field edits.',
        execution: 'latest',
        handle: async ({ prompt }, context) => {
          const current = context.snapshot()
          const result = await this.source.grokFill({
            formId: this.formId,
            prompt,
            fieldValues: cloneRecord(current.values),
            detailsText: current.detailsText,
          })
          if (!result.ok) {
            context.update((model) => ({ ...model, error: result.message }))
            throw new Error(result.message)
          }

          context.update((model) => {
            const values = applyDateDefaults(this.template, result.data.fieldValues)
            const suppliedBody = result.data.body?.trim() ? result.data.body : null
            return {
              ...model,
              values,
              detailsText: suppliedBody
                ?? (model.bodyEdited
                  ? model.detailsText
                  : documentBodyText(this.template, values, model.sections)),
              bodyEdited: suppliedBody ? true : model.bodyEdited,
              message: result.data.note,
              error: null,
            }
          })
          return result.data.note
        },
      },
      'formEditor.issue': {
        description: 'Persist any dirty draft, synchronize its service binding, then issue immutable vault bytes.',
        execution: 'serial',
        handle: async (_request, context) => this.issue(context),
      },
      'formEditor.sendSignature': {
        description: 'Synchronize the submitted draft and send the canonical issued document for signature.',
        execution: 'serial',
        handle: async (request, context) => {
          const model = context.snapshot()
          const sections = this.composedSections(model)
          const result = await this.source.sendSignature(this.formId, {
            ...request,
            fieldValues: cloneRecord(model.values),
            sections,
          })
          if (!result.ok) {
            context.update((current) => ({ ...current, error: result.message }))
            return null
          }

          const fingerprint = formContentFingerprint(model.values, sections)
          context.update((current) => ({
            ...current,
            issued: {
              documentId: result.data.documentId,
              issuedVersion: result.data.issuedVersion,
              checksum: current.issued?.checksum ?? '',
              contentFingerprint: fingerprint,
            },
            signatureState: {
              id: result.data.signatureRequestId,
              status: result.data.status,
            },
            message:
              `Sent for signature · ${result.data.signerCount} external ` +
              `part${result.data.signerCount === 1 ? 'y' : 'ies'} · ${result.data.signerName}` +
              (result.data.signerEmail ? ` · ${result.data.signerEmail}` : ''),
            error: null,
          }))
          return result.data
        },
      },
      'formEditor.feedback': {
        description: 'Publish browser-side action feedback into the shared Status model.',
        execution: 'parallel',
        handle: ({ message, error }, context) => {
          context.update((model) => ({
            ...model,
            message: message === undefined ? model.message : message,
            error: error === undefined ? model.error : error,
          }))
        },
      },
      'formEditor.busyChanged': {
        description: 'Reflect browser-only work such as native Share in the PageModel.',
        execution: 'parallel',
        handle: ({ busy }, context) => {
          context.update((model) => ({ ...model, busy }))
        },
      },
    }
  }

  private composedSections(model: Readonly<FormEditorPageModel>) {
    return {
      ...model.sections,
      body: model.detailsText,
      bodyEdited: model.bodyEdited ? 'true' : 'false',
    }
  }

  private async persistDraft(
    context: PageOperationContext<FormEditorPageModel>,
    quiet: boolean,
  ): Promise<boolean> {
    const before = context.snapshot()
    const values = cloneRecord(before.values)
    const detailsText = before.detailsText
    const sections = this.composedSections(before)

    context.update((model) => ({ ...model, draftSaving: true }))
    try {
      const result = await this.source.update(this.formId, values, sections)
      if (!result.ok) {
        context.update((model) => ({
          ...model,
          error: result.message,
          draftSaving: false,
        }))
        return false
      }

      context.update((model) => {
        const stillSame =
          JSON.stringify(model.values) === JSON.stringify(values) &&
          model.detailsText === detailsText
        return {
          ...model,
          sections,
          saved: stillSame
            ? { values, sections, detailsText }
            : model.saved,
          message: quiet ? model.message : 'Saved',
          error: null,
          draftSaving: false,
        }
      })
      return true
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Could not save.'
      context.update((model) => ({
        ...model,
        error: message,
        draftSaving: false,
      }))
      return false
    }
  }

  private async issue(
    context: PageOperationContext<FormEditorPageModel>,
  ): Promise<FormEditorIssuedDocument | null> {
    context.update((model) => ({ ...model, busy: true, error: null }))
    try {
      if (isFormEditorDirty(context.snapshot())) {
        const saved = await this.persistDraft(context, true)
        if (!saved) return null
      }

      const result = await this.source.issue(this.formId)
      if (!result.ok) {
        context.update((model) => ({ ...model, error: result.message }))
        return null
      }

      const current = context.snapshot()
      const document: FormEditorIssuedDocument = {
        ...result.data,
        contentFingerprint: formContentFingerprint(
          current.values,
          this.composedSections(current),
        ),
      }
      context.update((model) => ({
        ...model,
        issued: document,
        message: `Saved to vault v${document.issuedVersion}`,
        error: null,
      }))
      return document
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Could not save the PDF.'
      context.update((model) => ({ ...model, error: message }))
      return null
    } finally {
      context.update((model) => ({ ...model, busy: false }))
    }
  }
}
