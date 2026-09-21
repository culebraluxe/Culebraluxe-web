import { FormEditorSurface } from "@/components/portal/forms/form-editor-surface"

export const dynamic = "force-dynamic"

export default async function FormPage({
  params,
}: {
  params: Promise<{ formId: string }>
}) {
  const { formId } = await params
  return <FormEditorSurface formId={formId} />
}
