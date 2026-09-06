import type { FormEditorController } from './form-editor-controller'

const ACTIVE_CONTROLLERS = new Map<string, FormEditorController>()

/**
 * Tiny view-runtime bridge for controls that live next to the proven FormEditor
 * but still must dispatch through the same MVI controller. This avoids cloning
 * or re-authoring the mature editor surface just to add V4 controls.
 */
export function registerFormEditorController(
  formId: string,
  controller: FormEditorController,
): void {
  ACTIVE_CONTROLLERS.set(formId, controller)
}

export function getFormEditorController(
  formId: string,
): FormEditorController | null {
  return ACTIVE_CONTROLLERS.get(formId) ?? null
}
