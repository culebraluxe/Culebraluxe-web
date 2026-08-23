"use client"

import { useMemo, useState } from "react"

import {
  issueFormAction,
  updateFormAction,
} from "@/app/portal/forms/actions"
import {
  formatFieldValue,
  interpolateSectionText,
} from "@/lib/forms/pdf"
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

const DETAIL_SECTION_ORDER = [
  "additionalTerms",
  "specialTerms",
  "specialConditions",
  "amendments",
  "additional",
  "interest",
  "feedback",
  "followUp",
  "marketing",
  "access",
  "financingTerms",
  "appraisalSurveyInspection",
]

function pickDetailsSection(template: TemplateDefinition) {
  const editable = template.sections.filter((section) => section.editable)
  for (const name of DETAIL_SECTION_ORDER) {
    const match = editable.find((section) => section.name === name)
    if (match) return match
  }
  return editable[0] ?? null
}

function initialDetailsText(
  template: TemplateDefinition,
  sections: Record<string, string>,
) {
  const editable = template.sections.filter((section) => section.editable)
  const chunks = editable
    .map((section) => (sections[section.name] ?? "").trim())
    .filter(Boolean)
  return chunks.join("\n\n")
}

function issuedPdfUrl(documentId: string) {
  return `/portal/documents/${documentId}/download?inline=1`
}

async function pdfFileFromUrl(
  url: string,
  filename: string,
): Promise<File> {
  const response = await fetch(url)
  if (!response.ok) {
    throw new Error(`Failed to load PDF: ${response.status}`)
  }
  const buffer = await response.arrayBuffer()
  const header = String.fromCharCode(
    ...new Uint8Array(buffer).subarray(0, 5),
  )
  if (header !== "%PDF-") {
    throw new Error("Vault file is not a PDF.")
  }
  const safeFilename = filename.endsWith(".pdf")
    ? filename
    : `${filename}.pdf`
  return new File([buffer], safeFilename, {
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

async function writePdfToDisk(blob: Blob, filename: string) {
  const picker = window as unknown as {
    showSaveFilePicker?: (options: {
      suggestedName: string
      types: { description: string; accept: Record<string, string[]> }[]
    }) => Promise<{
      createWritable: () => Promise<{
        write: (data: Blob) => Promise<void>
        close: () => Promise<void>
      }>
    }>
  }
  if (typeof picker.showSaveFilePicker === "function") {
    const handle = await picker.showSaveFilePicker({
      suggestedName: filename,
      types: [
        {
          description: "PDF",
          accept: { "application/pdf": [".pdf"] },
        },
      ],
    })
    const writable = await handle.createWritable()
    await writable.write(blob)
    await writable.close()
    return
  }
  const objectUrl = URL.createObjectURL(blob)
  const link = document.createElement("a")
  link.href = objectUrl
  link.download = filename
  document.body.appendChild(link)
  link.click()
  link.remove()
  URL.revokeObjectURL(objectUrl)
}

export function FormEditor({
  form,
  template,
  issuedDocument = null,
}: {
  form: {
    id: string
    status: string
    fieldValues: Record<string, string>
    sections: Record<string, string>
  }
  template: TemplateDefinition
  issuedDocument?: {
    documentId: string
    issuedVersion: number
    checksum: string
  } | null
}) {
  const detailsSection = useMemo(
    () => pickDetailsSection(template),
    [template],
  )
  const [values, setValues] = useState<Record<string, string>>(form.fieldValues)
  const [sections, setSections] = useState<Record<string, string>>(form.sections)
  const [detailsText, setDetailsText] = useState(() =>
    initialDetailsText(template, form.sections),
  )
  const [saved, setSaved] = useState({
    values: form.fieldValues,
    sections: form.sections,
    detailsText: initialDetailsText(template, form.sections),
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
  const isIssued = form.status === "issued" || issued !== null
  const boilerplate = template.sections.filter((section) => !section.editable)
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
    const next = { ...sections }
    for (const section of template.sections) {
      if (!section.editable) continue
      next[section.name] = ""
    }
    if (detailsSection) {
      next[detailsSection.name] = nextDetails
    }
    return next
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

  async function ensureIssuedDocument() {
    if (issued) return issued
    if (dirty) {
      const savedOk = await saveDraft()
      if (!savedOk) {
        throw new Error("Could not save the form before creating the PDF.")
      }
    }
    const result = await issueFormAction(form.id)
    if (!result.ok) {
      throw new Error(result.message ?? "Could not issue the PDF.")
    }
    setIssued(result.data)
    return result.data
  }

  async function vaultPdfFile() {
    const document = await ensureIssuedDocument()
    return pdfFileFromUrl(issuedPdfUrl(document.documentId), pdfFilename())
  }

  async function savePdf() {
    setError(null)
    setBusy(true)
    try {
      const file = await vaultPdfFile()
      await writePdfToDisk(file, file.name)
      setMessage("PDF saved")
    } catch (caught) {
      if (isUserCancel(caught)) return
      setError(caught instanceof Error ? caught.message : "Could not save the PDF.")
    } finally {
      setBusy(false)
    }
  }

  async function sharePdf() {
    setError(null)
    setBusy(true)
    try {
      const file = await vaultPdfFile()
      if (!navigator.canShare?.({ files: [file] })) {
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
      setError(caught instanceof Error ? caught.message : "Could not share the PDF.")
    } finally {
      setBusy(false)
    }
  }

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <p className="text-[10px] font-light uppercase tracking-[0.18em] text-black/40">
            {template.rendering.issuer}
          </p>
          <h1 className="font-serif text-xl font-light text-[var(--portal-navy)]">
            {template.displayName}
          </h1>
        </div>
        <span className="text-[10px] font-light uppercase tracking-[0.14em] text-black/40">
          {isIssued ? "Issued" : dirty ? "Unsaved" : form.status} · v
          {template.version}
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
                      setValues({
                        ...values,
                        [field.name]: event.target.value,
                      })
                    }
                    disabled={isIssued}
                    className={`${inputClass} h-auto min-h-12 resize-y py-1.5`}
                  />
                ) : field.type === "select" ? (
                  <select
                    value={values[field.name] ?? ""}
                    onChange={(event) =>
                      setValues({
                        ...values,
                        [field.name]: event.target.value,
                      })
                    }
                    disabled={isIssued}
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
                      setValues({
                        ...values,
                        [field.name]: event.target.value,
                      })
                    }
                    disabled={isIssued}
                    className={inputClass}
                  />
                )}
              </label>
            ))}
          </div>

          <div className="mt-6">
            <h2 className={sectionHeadingClass}>Deal details</h2>
            <textarea
              id="deal-details"
              rows={8}
              value={detailsText}
              placeholder="Special terms, conditions, and anything else that belongs in this document…"
              onChange={(event) => setDetailsText(event.target.value)}
              disabled={isIssued}
              style={{ minHeight: 160 }}
              className="mt-2 block w-full resize-y rounded-[var(--portal-tab-radius)] border-2 border-[var(--portal-navy)] bg-white px-3 py-2 text-[14px] font-light leading-6 text-black/80 outline-none focus:border-[var(--portal-navy-soft)] disabled:opacity-60"
            />
          </div>

          <div className="mt-4 flex flex-wrap items-center gap-2">
            <button
              type="button"
              disabled={working || isIssued}
              onClick={() => {
                void (async () => {
                  setError(null)
                  setBusy(true)
                  try {
                    const file = await vaultPdfFile()
                    setMessage(`PDF saved (${Math.round(file.size / 1024)} KB)`)
                  } catch (caught) {
                    if (isUserCancel(caught)) return
                    setError(
                      caught instanceof Error
                        ? caught.message
                        : "Could not save the PDF.",
                    )
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
              disabled={working || isIssued || !dirty}
              onClick={cancelEdits}
              className={secondaryButton}
            >
              Cancel
            </button>
            {dirty && !isIssued ? (
              <span className="text-xs font-light text-black/45">
                Unsaved changes
              </span>
            ) : null}
          </div>
        </section>

        <div className="flex flex-col gap-3 lg:sticky lg:top-4">
        <section className="portal-glass-panel overflow-hidden rounded-[var(--portal-panel-radius)]">
          {issued ? (
            <iframe
              title="Issued PDF"
              src={issuedPdfUrl(issued.documentId)}
              className="min-h-[70vh] w-full bg-white lg:h-[calc(100vh-9.5rem)]"
            />
          ) : (
          <div className="max-h-[calc(100vh-6rem)] overflow-y-auto bg-white px-8 py-7 text-black/80 lg:max-h-[calc(100vh-6.5rem)]">
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

            {boilerplate.map((section) => {
              const text = interpolateSectionText(
                section,
                values,
                formatFieldValue,
                template.fields,
              )
              if (!text) return null
              return (
                <article key={section.name} className="mt-6">
                  <h3 className={sectionHeadingClass}>{section.label}</h3>
                  <p className="mt-2 whitespace-pre-wrap text-[14px] font-light leading-7">
                    {text}
                  </p>
                </article>
              )
            })}

            {detailsText.trim() ? (
              <article className="mt-6">
                <h3 className={sectionHeadingClass}>
                  {detailsSection?.label ?? "Details"}
                </h3>
                <p className="mt-2 whitespace-pre-wrap text-[14px] font-light leading-7">
                  {detailsText}
                </p>
              </article>
            ) : null}

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
          )}
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
