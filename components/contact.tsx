'use client'

import { useRef, useState, type FormEvent } from 'react'
import { Reveal } from '@/components/reveal'
import { submitWebsitePropertyIntake } from '@/app/actions/website-intake'

const INTERESTS = ['Buying', 'Selling', 'Both'] as const
type Interest = (typeof INTERESTS)[number]

type PropertyContext = {
  propertyId?: string
  requestType: 'private_viewing' | 'property_information'
}

export function Contact({ propertyContext }: { propertyContext?: PropertyContext }) {
  const [interest, setInterest] = useState<Interest>('Buying')
  const [submitted, setSubmitted] = useState(false)
  const [pending, setPending] = useState(false)
  const [error, setError] = useState(false)
  const submissionId = useRef<string | null>(null)

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    setError(false)

    // The generic site contact remains the existing client-only experience.
    if (!propertyContext?.propertyId) {
      setSubmitted(true)
      return
    }

    submissionId.current ??= crypto.randomUUID()
    const formData = new FormData(event.currentTarget)
    formData.set('submissionId', submissionId.current)
    formData.set('propertyId', propertyContext.propertyId)
    formData.set('requestType', propertyContext.requestType)

    setPending(true)
    try {
      const result = await submitWebsitePropertyIntake(formData)
      if (result.accepted) setSubmitted(true)
      else setError(true)
    } catch {
      setError(true)
    } finally {
      setPending(false)
    }
  }

  return (
    <section id="contact" className="bg-primary px-6 py-28 text-primary-foreground md:px-12 md:py-40">
      <div className="mx-auto grid max-w-[1600px] gap-16 md:grid-cols-12 md:gap-24">
        <Reveal className="md:col-span-5">
          <p className="mb-6 text-xs font-light uppercase tracking-[0.34em] text-primary-foreground/50">
            Contact
          </p>
          <h2 className="text-balance font-serif text-4xl font-light leading-[1.06] md:text-6xl">
            Let&apos;s begin a quiet conversation.
          </h2>
          <div className="mt-14 flex flex-col gap-8 border-t border-primary-foreground/10 pt-10">
            <div>
              <p className="text-xs font-light uppercase tracking-[0.2em] text-primary-foreground/45">
                Office
              </p>
              <p className="mt-2 text-sm font-light text-primary-foreground/85">
                Calle Escudero, Dewey, Culebra, PR 00775
              </p>
            </div>
            <div>
              <p className="text-xs font-light uppercase tracking-[0.2em] text-primary-foreground/45">
                Enquiries
              </p>
              <a
                href="mailto:hello@culebraluxe.com"
                className="mt-2 inline-block text-sm font-light text-primary-foreground/85 transition-colors hover:text-primary-foreground"
              >
                hello@culebraluxe.com
              </a>
            </div>
          </div>
        </Reveal>

        <Reveal delay={120} className="md:col-span-7">
          {submitted ? (
            <div className="flex h-full min-h-64 flex-col items-start justify-center border-t border-primary-foreground/10 pt-10">
              <p className="font-serif text-3xl font-light md:text-4xl">Thank you.</p>
              <p className="mt-4 max-w-md text-sm font-light leading-relaxed text-primary-foreground/70">
                Your note has reached us. A member of the CulebraLuxe team will respond
                personally within one business day.
              </p>
            </div>
          ) : (
            <form onSubmit={handleSubmit} className="flex flex-col gap-10">
              <div className="absolute -left-[9999px]" aria-hidden="true">
                <label htmlFor="company">Company</label>
                <input id="company" name="company" type="text" tabIndex={-1} autoComplete="off" />
              </div>
              <div className="grid gap-10 sm:grid-cols-2">
                <Field id="name" label="Name" type="text" autoComplete="name" required />
                <Field id="email" label="Email" type="email" autoComplete="email" required />
              </div>

              <fieldset className="flex flex-col gap-4">
                <legend className="text-xs font-light uppercase tracking-[0.22em] text-primary-foreground/50">
                  I am interested in
                </legend>
                <div className="flex flex-wrap gap-3">
                  {INTERESTS.map((option) => (
                    <button
                      type="button"
                      key={option}
                      onClick={() => setInterest(option)}
                      className={`border px-6 py-2.5 text-xs font-light uppercase tracking-[0.18em] transition-colors duration-300 ${
                        interest === option
                          ? 'border-primary-foreground bg-primary-foreground text-primary'
                          : 'border-primary-foreground/25 text-primary-foreground/70 hover:border-primary-foreground/60'
                      }`}
                      aria-pressed={interest === option}
                    >
                      {option}
                    </button>
                  ))}
                </div>
              </fieldset>

              <div className="flex flex-col gap-3">
                <label
                  htmlFor="message"
                  className="text-xs font-light uppercase tracking-[0.22em] text-primary-foreground/50"
                >
                  Message
                </label>
                <textarea
                  id="message"
                  name="message"
                  rows={4}
                  className="resize-none border-0 border-b border-primary-foreground/25 bg-transparent pb-3 text-sm font-light text-primary-foreground placeholder:text-primary-foreground/30 focus:border-primary-foreground focus:outline-none"
                  placeholder="Tell us a little about what you are looking for."
                />
              </div>

              <button
                type="submit"
                disabled={pending}
                className="group mt-2 inline-flex items-center gap-3 self-start text-xs font-light uppercase tracking-[0.24em]"
              >
                {pending ? 'Sending…' : 'Send enquiry'}
                <span className="inline-block h-px w-12 bg-primary-foreground transition-all duration-500 group-hover:w-20" />
              </button>
              {error ? (
                <p className="text-sm text-primary-foreground/70" role="alert">
                  We could not send your note. Please try again.
                </p>
              ) : null}
            </form>
          )}
        </Reveal>
      </div>
    </section>
  )
}

function Field({
  id,
  label,
  type,
  autoComplete,
  required,
}: {
  id: string
  label: string
  type: string
  autoComplete?: string
  required?: boolean
}) {
  return (
    <div className="flex flex-col gap-3">
      <label
        htmlFor={id}
        className="text-xs font-light uppercase tracking-[0.22em] text-primary-foreground/50"
      >
        {label}
      </label>
      <input
        id={id}
        name={id}
        type={type}
        autoComplete={autoComplete}
        required={required}
        className="border-0 border-b border-primary-foreground/25 bg-transparent pb-3 text-sm font-light text-primary-foreground placeholder:text-primary-foreground/30 focus:border-primary-foreground focus:outline-none"
      />
    </div>
  )
}
