import { BasePageController, type PageOperationDefinitions } from '@/ui/runtime'
import type { TemplateDefinition } from '@/lib/forms/template-types'
import {
  buildPnsLensModel,
  type PnsLensIntentMap,
  type PnsLensModel,
} from './model'

export class PnsLensController extends BasePageController<PnsLensModel, PnsLensIntentMap> {
  protected readonly operations: PageOperationDefinitions<PnsLensModel, PnsLensIntentMap>

  constructor(private readonly template: TemplateDefinition) {
    super(buildPnsLensModel(template))

    this.operations = {
      'pnsLens.fieldChanged': {
        description: 'Edit one P&S working field without mutating canonical or issued data.',
        handle: ({ field, value }, context) => {
          context.update((model) => ({
            ...model,
            fields: model.fields.map((candidate) =>
              candidate.definition.name === field ? { ...candidate, value } : candidate,
            ),
          }))
        },
      },
      'pnsLens.ownerChanged': {
        description: 'Filter the architecture lens by canonical owner.',
        handle: ({ owner }, context) => {
          context.update((model) => ({ ...model, ownerFilter: owner }))
        },
      },
      'pnsLens.reset': {
        description: 'Reset sidecar working values to the active template shape.',
        handle: (_request, context) => {
          const fresh = buildPnsLensModel(this.template)
          context.update((model) => ({ ...fresh, ownerFilter: model.ownerFilter }))
        },
      },
    }
  }
}
