"use client"

import { useRef, useState } from "react"
import { useRouter } from "next/navigation"

import {
  createFormAction,
  issueFormAction,
  updateFormAction,
} from "@/app/portal/forms/actions"
import { documentBodyText, formatFieldValue } from "@/lib/forms/format"
import type { TemplateDefinition } from "@/lib/forms/template-types"

const inputClass =
  "mt-1 block h-9 w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white px-2.5 text-[13px] font-light text-black/70 outline-none focus:border-[var(--portal-navy-soft)]"
const sectionHeadingClass =
  "font-serif text-base font-bold text-[var(--portal-navy)]"
const labelClass =
  "text-[9px] font-light uppercase tracking-[0.14em] text-black/40"
const primaryButton =
  "inline-flex min-h-8 items-center justify-center rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-3 text-[10px] font-medium uppercase tracking-[0.14em] text-white transition hover:bg-[var(--portal-navy-soft)] disabled:cursor-not-allowed disabled:opacity-40"
const secondaryButton =
  "inline-flex min-h-8 items-center justify-center rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] px-3 text-[10px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy-soft)] transition hover:border-[var(--portal-navy)] hover:text-[var(--portal-navy)] disabled:opacity-40"

function fieldSpanClass(field: { name: string; label: string; type: string }) {
  if (field.type === "textarea") return "col-span-6"
  if (
    field.type === "date" ||
    field.type === "money" ||
    field.type === "select"
  ) {
    return "col-span-2"
  }
  if (/name|property|location|address/i.test(`${field.name} ${field.label}`)) {
    return "col-span-3"
  }
  return "col-span-2"
}

function initialDetailsText(
  template: TemplateDefinition,
  sections: Record<string, string>,
  values: Record<string, string>,
) {
  if (sections.body?.trim()) return sections.body
  return documentBodyText(template, values)
}

function fileFromPdfBytes(buffer: ArrayBuffer, filename: string): File {
  const bytes = new Uint8Array(buffer)
  const header = String.fromCharCode(...bytes.subarray(0, 5))
  if (header !== "%PDF-") {
    throw new Error("The generated file was not a PDF.")
  }
  const safeFilename = filename.endsWith(".pdf")
    ? filename
    : `${filename}.pdf`
  return new File([bytes], safeFilename, {
    type: "application/pdf",
    lastModified: Date.now(),
  })
}

function fileSafeName(value: string) {
  const cleaned = value
    .replace(/[^\w\s-]+/g, "")
    .trim()
    .replace(/\s+/g, "-")
  return cleaned || "form"
}

function isUserCancel(error: unknown) {
  return error instanceof DOMException && error.name === "AbortError"
}

export function FormEditor({
  form,
  template,
  templates,
  issuedDocument = null,
}: {
  form: {
    id: string
    status: string
    templateId: string
    dealId: string | null
    personId: string | null
    propertyId: string | null
    fieldValues: Record<string, string>
    sections: Record<string, string>
  }
  template: TemplateDefinition
  templates: { id: string; displayName: string }[]
  issuedDocument?: {
    documentId: string
    issuedVersion: number
    checksum: string
  } | null
}) {
  const router = useRouter()
  const [values, setValues] = useState<Record<string, string>>(form.fieldValues)
  const [sections, setSections] = useState<Record<string, string>>(form.sections)
  const [detailsText, setDetailsText] = useState(() =>
    initialDetailsText(template, form.sections, form.fieldValues),
  )
  const bodyTouched = useRef(Boolean(form.sections.body?.trim()))
  const [saved, setSaved] = useState({
    values: form.fieldValues,
    sections: form.sections,
    detailsText: initialDetailsText(
      template,
      form.sections,
      form.fieldValues,
    ),
  })
  const [message, setMessage] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [issued, setIssued] = useState<{
    documentId: string
    issuedVersion: number
    checksum: string
  } | null>(issuedDocument)
  const [busy, setBusy] = useState(false)
  const working = busy
  const vaultVersion = issued?.issuedVersion ?? null
  const signatureGroups =
    template.signatureGroups.length > 0
      ? template.signatureGroups
      : [
          {
            role: "PARTY",
            label: "Signature",
            field: template.fields[0]?.name ?? null,
            initials: false,
          },
        ]
  const dirty =
    JSON.stringify(values) !== JSON.stringify(saved.values) ||
    detailsText !== saved.detailsText

  function composedSections(
    nextDetails = detailsText,
  ): Record<string, string> {
    return { ...sections, body: nextDetails }
  }

  function updateField(name: string, value: string) {
    const next = { ...values, [name]: value }
    setValues(next)
    if (!bodyTouched.current) {
      setDetailsText(documentBodyText(template, next))
    }
  }

  async function saveDraft(
    nextValues = values,
    nextDetails = detailsText,
  ): Promise<boolean> {
    const nextSections = composedSections(nextDetails)
    const result = await updateFormAction(form.id, nextValues, nextSections)
    if (result.ok) {
      setSections(nextSections)
      setSaved({
        values: nextValues,
        sections: nextSections,
        detailsText: nextDetails,
      })
      setMessage("Saved")
      setError(null)
      return true
    }
    setError(result.message ?? "Could not save.")
    return false
  }

  function cancelEdits() {
    setValues(saved.values)
    setSections(saved.sections)
    setDetailsText(saved.detailsText)
    setMessage("Changes discarded")
    setError(null)
  }

  function pdfFilename() {
    const who =
      values.buyerName ||
      values.sellerName ||
      values.clientName ||
      template.displayName
    return `${fileSafeName(who)}-${fileSafeName(template.id)}.pdf`
  }

  async function savePdfToVault() {
    if (dirty) {
      const savedOk = await saveDraft()
      if (!savedOk) {
        throw new Error("Could not save the form before creating the PDF.")
      }
    }
    const result = await issueFormAction(form.id)
    if (!result.ok) {
      throw new Error(result.message ?? "Could not save the PDF to the vault.")
    }
    setIssued(result.data)
    return result.data
  }

  async function livePdfFile() {
    const response = await fetch(`/portal/forms/${form.id}/preview`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        fieldValues: values,
        sections: composedSections(),
      }),
    })
    if (!response.ok) {
      throw new Error("Could not build the PDF.")
    }
    return fileFromPdfBytes(await response.arrayBuffer(), pdfFilename())
  }

  async function savePdf() {
    setError(null)
    setBusy(true)
    try {
      const document = await savePdfToVault()
      setMessage(`Saved to vault v${document.issuedVersion}`)
    } catch (caught) {
      if (isUserCancel(caught)) return
      setError(caught instanceof Error ? caught.message : "Could not save the PDF.")
    } finally {
      setBusy(false)
    }
  }

  async function sharePdf() {
    setError(null)
    try {
      const file = await livePdfFile()
      if (typeof navigator.share !== "function") {
        setMessage(
          "This browser can't attach a PDF from the page. Save PDF, then attach that file in Mail or Messages.",
        )
        return
      }
      await navigator.share({
        title: "CulebraLuxe Document",
        text: "CulebraLuxe transaction document",
        files: [file],
      })
      setMessage("Shared")
    } catch (caught) {
      if (isUserCancel(caught)) return
      const name = caught instanceof DOMException ? caught.name : ""
      if (name === "NotAllowedError" || name === "TypeError") {
        setMessage(
          "Share needs a direct click. Try Share PDF again, or Save PDF and attach the file.",
        )
        return
      }
      setError(caught instanceof Error ? caught.message : "Could not share the PDF.")
    }
  }

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <label className="min-w-[16rem] flex-1">
          <span className={labelClass}>Form</span>
          <select
            value={form.templateId}
            disabled={working}
            onChange={(event) => {
              const nextTemplateId = event.target.value
              if (nextTemplateId === form.templateId) return
              setBusy(true)
              void (async () => {
                const result = await createFormAction({
                  templateId: nextTemplateId,
                  dealId: form.dealId ?? undefined,
                  personId: form.personId ?? undefined,
                  propertyId: form.propertyId ?? undefined,
                })
                if (result.ok) router.push(`/portal/forms/${result.data.formId}`)
                else {
                  setError(result.message ?? "Could not switch forms.")
                  setBusy(false)
                }
              })()
            }}
            className={`${inputClass} max-w-md font-serif text-base text-[var(--portal-navy)]`}
          >
            {templates.map((item) => (
              <option key={item.id} value={item.id}>
                {item.displayName}
              </option>
            ))}
          </select>
        </label>
        <span className="pb-2 text-[10px] font-light uppercase tracking-[0.14em] text-black/40">
          {dirty ? "Unsaved" : vaultVersion ? `Vault v${vaultVersion}` : "Draft"}
        </span>
      </div>

      {error ? (
        <p className="text-xs font-light text-[var(--portal-archive)]">{error}</p>
      ) : null}

      <div className="grid items-start gap-4 lg:grid-cols-2">
        <section className="portal-glass-panel rounded-[var(--portal-panel-radius)] p-5">
          <div className="grid grid-cols-6 gap-x-3 gap-y-3.5">
            {template.fields.map((field) => (
              <label
                key={field.name}
                className={`${fieldSpanClass(field)} min-w-0`}
              >
                <span className={labelClass}>
                  {field.label}
                  {field.required ? " *" : ""}
                </span>
                {field.type === "textarea" ? (
                  <textarea
                    rows={2}
                    value={values[field.name] ?? ""}
                    onChange={(event) =>
                      updateField(field.name, event.target.value)
                    }
                    className={`${inputClass} h-auto min-h-12 resize-y py-1.5`}
                  />
                ) : field.type === "select" ? (
                  <select
                    value={values[field.name] ?? ""}
                    onChange={(event) =>
                      updateField(field.name, event.target.value)
                    }
                    className={inputClass}
                  >
                    <option value="">—</option>
                    {(field.options ?? []).map((option) => (
                      <option key={option} value={option}>
                        {option}
                      </option>
                    ))}
                  </select>
                ) : (
                  <input
                    type={field.type === "date" ? "date" : "text"}
                    value={values[field.name] ?? ""}
                    onChange={(event) =>
                      updateField(field.name, event.target.value)
                    }
                    className={inputClass}
                  />
                )}
              </label>
            ))}
          </div>

          <div className="mt-6">
            <h2 className={sectionHeadingClass}>Document</h2>
            <p className="mt-1 text-xs font-light text-black/40">
              Template text from the form. Edit it like a Word document.
            </p>
            <textarea
              id="deal-details"
              rows={18}
              value={detailsText}
              placeholder="Document text…"
              onChange={(event) => {
                bodyTouched.current = true
                setDetailsText(event.target.value)
              }}
              style={{ minHeight: 360 }}
              className="mt-2 block w-full resize-y rounded-[var(--portal-tab-radius)] border-2 border-[var(--portal-navy)] bg-white px-3 py-2 font-serif text-[15px] font-light leading-7 text-black/80 outline-none focus:border-[var(--portal-navy-soft)] disabled:opacity-60"
            />
          </div>

          <div className="mt-4 flex flex-wrap items-center gap-2">
            <button
              type="button"
              disabled={working || !dirty}
              onClick={() => {
                void (async () => {
                  setError(null)
                  setBusy(true)
                  try {
                    await saveDraft()
                  } finally {
                    setBusy(false)
                  }
                })()
              }}
              className={primaryButton}
            >
              {working ? "Working…" : "Save"}
            </button>
            <button
              type="button"
              disabled={working || !dirty}
              onClick={cancelEdits}
              className={secondaryButton}
            >
              Cancel
            </button>
            {dirty ? (
              <span className="text-xs font-light text-black/45">
                Unsaved changes
              </span>
            ) : null}
          </div>
        </section>

        <div className="flex flex-col gap-3 lg:sticky lg:top-4">
        <section className="portal-glass-panel overflow-hidden rounded-[var(--portal-panel-radius)]">
          <div className="max-h-[calc(100vh-6rem)] overflow-y-auto bg-white px-8 py-7 text-black/80 lg:max-h-[calc(100vh-9.5rem)]">
            <p className="text-[9px] font-light uppercase tracking-[0.18em] text-black/40">
              {template.rendering.issuer}
            </p>
            <h2 className="mt-1 font-serif text-xl font-bold text-[var(--portal-navy)]">
              {template.rendering.title}
            </h2>

            <p className="my-5 border-y border-black/10 py-4 font-serif text-[15px] font-light leading-7">
              {template.fields
                .map((field) => {
                  const raw = (values[field.name] ?? "").trim()
                  if (!raw) return null
                  return formatFieldValue(field, raw)
                })
                .filter((part): part is string => Boolean(part))
                .join(" · ")}
            </p>

            {detailsText.split(/\n{2,}/).map((block, index) => {
              const lines = block.split("\n")
              const heading = lines[0]?.trim() ?? ""
              const para = lines.slice(1).join("\n").trim()
              if (!heading && !para) return null
              return (
                <article key={index} className="mt-5">
                  {para ? (
                    <>
                      <h3 className={sectionHeadingClass}>{heading}</h3>
                      <p className="mt-2 whitespace-pre-wrap text-[14px] font-light leading-7">
                        {para}
                      </p>
                    </>
                  ) : (
                    <p className="whitespace-pre-wrap text-[14px] font-light leading-7">
                      {heading}
                    </p>
                  )}
                </article>
              )
            })}

            <div className="mt-10 border-t border-black/20 pt-6">
              <h3 className={sectionHeadingClass}>Signatures</h3>
              <div className="mt-4 grid gap-6 sm:grid-cols-2">
                {signatureGroups.map((group) => {
                  const name = group.field
                    ? (values[group.field] ?? "").trim()
                    : ""
                  return (
                    <div key={`${group.role}-${group.label}`}>
                      <p className={labelClass}>{group.label}</p>
                      <p className="mt-0.5 font-serif text-sm font-light text-[var(--portal-navy)]">
                        {name || "Name on file"}
                      </p>
                      <div className="mt-5 border-b border-black/45 pb-0.5 text-[10px] font-light text-black/40">
                        Signature
                      </div>
                      <div className="mt-4 flex items-end gap-5">
                        {group.initials ? (
                          <div className="w-20 border-b border-black/45 pb-0.5 text-[10px] font-light text-black/40">
                            Initials
                          </div>
                        ) : null}
                        <div className="min-w-[7rem] flex-1 border-b border-black/45 pb-0.5 text-[10px] font-light text-black/40">
                          Date
                        </div>
                      </div>
                    </div>
                  )
                })}
              </div>
            </div>
          </div>
        </section>
        <div className="flex flex-wrap items-center gap-2">
          <button
            type="button"
            disabled={working}
            onClick={() => {
              void savePdf()
            }}
            className={primaryButton}
          >
            {working ? "Working…" : "Save PDF"}
          </button>
          <button
            type="button"
            disabled={working}
            onClick={() => {
              void sharePdf()
            }}
            className={secondaryButton}
          >
            Share PDF
          </button>
          {message ? (
            <span className="text-xs font-light text-black/45">{message}</span>
          ) : null}
        </div>
        </div>
      </div>
    </div>
  )
}
