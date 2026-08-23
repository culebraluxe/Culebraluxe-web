import type { TemplateDefinition } from "@/lib/forms/template-types"

export type GrokFillResult = {
  fieldValues: Record<string, string>
  body: string | null
  note: string
}

export function parseGrokFillJson(raw: string): GrokFillResult {
  const trimmed = raw.trim()
  const fenced = trimmed.match(/```(?:json)?\s*([\s\S]*?)```/)
  const jsonText = fenced?.[1]?.trim() ?? trimmed
  const start = jsonText.indexOf("{")
  const end = jsonText.lastIndexOf("}")
  if (start < 0 || end <= start) {
    throw new Error("Grok did not return form data.")
  }
  const parsed = JSON.parse(jsonText.slice(start, end + 1)) as {
    fieldValues?: unknown
    body?: unknown
    note?: unknown
  }
  const fieldValues: Record<string, string> = {}
  if (parsed.fieldValues && typeof parsed.fieldValues === "object") {
    for (const [key, value] of Object.entries(parsed.fieldValues)) {
      if (typeof value === "string" && value.trim()) {
        fieldValues[key] = value.trim()
      }
    }
  }
  return {
    fieldValues,
    body: typeof parsed.body === "string" && parsed.body.trim() ? parsed.body : null,
    note:
      typeof parsed.note === "string" && parsed.note.trim()
        ? parsed.note.trim()
        : "Filled from what you told Grok.",
  }
}

export function applyGrokFields(
  template: TemplateDefinition,
  current: Record<string, string>,
  suggested: Record<string, string>,
): Record<string, string> {
  const next = { ...current }
  for (const field of template.fields) {
    const value = suggested[field.name]
    if (!value) continue
    if (field.type === "select") {
      const match = (field.options ?? []).find(
        (option) => option.toLowerCase() === value.toLowerCase(),
      )
      if (match) next[field.name] = match
      continue
    }
    next[field.name] = value
  }
  return next
}

export async function requestGrokFormFill(input: {
  prompt: string
  template: TemplateDefinition
  fieldValues: Record<string, string>
  detailsText: string
}): Promise<GrokFillResult> {
  const apiKey = process.env.XAI_API_KEY?.trim()
  if (!apiKey) {
    throw new Error("Grok is not configured on this server yet.")
  }

  const fields = input.template.fields.map((field) => ({
    name: field.name,
    label: field.label,
    type: field.type,
    options: field.options ?? [],
    current: input.fieldValues[field.name] ?? "",
  }))

  const response = await fetch("https://api.x.ai/v1/chat/completions", {
    method: "POST",
    headers: {
      Authorization: `Bearer ${apiKey}`,
      "Content-Type": "application/json",
    },
    body: JSON.stringify({
      model: "grok-4.6",
      temperature: 0.2,
      messages: [
        {
          role: "system",
          content: `You fill CulebraLuxe real-estate forms. Return ONLY JSON:
{"fieldValues":{"fieldName":"value"},"body":"optional document prose","note":"one short sentence for the agent"}
Rules:
- Only include fields you are confident about.
- Dates must be YYYY-MM-DD.
- Select fields must use one of the given options.
- Do not invent emails, legal clauses, or prices the user did not state.
- Keep existing values unless the user is changing them.
- body is optional; only replace the document body if the user described terms or narrative.`,
        },
        {
          role: "user",
          content: JSON.stringify({
            form: input.template.displayName,
            fields,
            documentBody: input.detailsText,
            agentSaid: input.prompt,
          }),
        },
      ],
    }),
  })

  if (!response.ok) {
    throw new Error("Grok could not fill the form right now. Try again in a moment.")
  }
  const payload = (await response.json()) as {
    choices?: Array<{ message?: { content?: string } }>
  }
  const content = payload.choices?.[0]?.message?.content
  if (!content) throw new Error("Grok did not return form data.")
  return parseGrokFillJson(content)
}
