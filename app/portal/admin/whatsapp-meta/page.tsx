export const dynamic = "force-dynamic"

const DEFAULT_WABA_ID = "1605543247626812"
const GRAPH_VERSION = "v23.0"

type MetaPhoneNumber = {
  id?: string
  display_phone_number?: string
  verified_name?: string
  quality_rating?: string
  code_verification_status?: string
}

type MetaPhoneResponse = {
  data?: MetaPhoneNumber[]
  error?: {
    message?: string
    type?: string
    code?: number
  }
}

async function getMetaPhones(): Promise<{
  wabaId: string
  phones: MetaPhoneNumber[]
  error: string | null
  tokenConfigured: boolean
}> {
  const wabaId = process.env.WHATSAPP_WABA_ID?.trim() || DEFAULT_WABA_ID
  const token = process.env.WHATSAPP_ACCESS_TOKEN?.trim()

  if (!token) {
    return {
      wabaId,
      phones: [],
      error: "WHATSAPP_ACCESS_TOKEN is not configured in Vercel Production.",
      tokenConfigured: false,
    }
  }

  try {
    const response = await fetch(
      `https://graph.facebook.com/${GRAPH_VERSION}/${encodeURIComponent(wabaId)}/phone_numbers`,
      {
        headers: {
          Authorization: `Bearer ${token}`,
        },
        cache: "no-store",
      },
    )

    const payload = (await response.json()) as MetaPhoneResponse

    if (!response.ok) {
      return {
        wabaId,
        phones: [],
        error: payload.error?.message || `Meta returned HTTP ${response.status}.`,
        tokenConfigured: true,
      }
    }

    return {
      wabaId,
      phones: payload.data ?? [],
      error: null,
      tokenConfigured: true,
    }
  } catch (error) {
    return {
      wabaId,
      phones: [],
      error: error instanceof Error ? error.message : "Unable to query Meta.",
      tokenConfigured: true,
    }
  }
}

export default async function WhatsAppMetaDiagnosticPage() {
  const result = await getMetaPhones()

  return (
    <main className="min-h-screen bg-brand-navy px-6 py-12 text-brand-ivory">
      <div className="mx-auto max-w-3xl space-y-8">
        <div>
          <p className="text-xs uppercase tracking-[0.28em] text-brand-gold">
            Private diagnostic
          </p>
          <h1 className="mt-3 font-serif text-3xl">WhatsApp Meta IDs</h1>
          <p className="mt-3 text-sm text-brand-ivory/70">
            This page queries Meta server-side. The access token is never sent to the browser.
          </p>
        </div>

        <section className="rounded-2xl border border-brand-gold/25 bg-white/5 p-6">
          <dl className="grid gap-4 sm:grid-cols-2">
            <div>
              <dt className="text-xs uppercase tracking-wider text-brand-ivory/50">WABA ID</dt>
              <dd className="mt-1 font-mono text-sm">{result.wabaId}</dd>
            </div>
            <div>
              <dt className="text-xs uppercase tracking-wider text-brand-ivory/50">Meta token</dt>
              <dd className="mt-1 text-sm">{result.tokenConfigured ? "Configured" : "Missing"}</dd>
            </div>
          </dl>
        </section>

        {result.error ? (
          <section className="rounded-2xl border border-amber-400/30 bg-amber-400/10 p-6">
            <h2 className="font-medium">Meta query not available yet</h2>
            <p className="mt-2 text-sm text-brand-ivory/80">{result.error}</p>
            {!result.tokenConfigured ? (
              <p className="mt-4 text-sm text-brand-ivory/70">
                Add one Vercel Production variable named <code>WHATSAPP_ACCESS_TOKEN</code>, then redeploy.
              </p>
            ) : null}
          </section>
        ) : null}

        {!result.error && result.phones.length === 0 ? (
          <section className="rounded-2xl border border-brand-gold/25 bg-white/5 p-6">
            <p className="text-sm">Meta returned no phone numbers for this WABA.</p>
          </section>
        ) : null}

        {result.phones.map((phone) => (
          <section
            key={phone.id ?? phone.display_phone_number ?? "phone"}
            className="rounded-2xl border border-brand-gold/25 bg-white/5 p-6"
          >
            <p className="text-xs uppercase tracking-[0.22em] text-brand-gold">WhatsApp number</p>
            <h2 className="mt-2 text-2xl font-medium">{phone.display_phone_number ?? "Unknown"}</h2>
            <div className="mt-6 grid gap-4 sm:grid-cols-2">
              <div>
                <p className="text-xs uppercase tracking-wider text-brand-ivory/50">Phone Number ID</p>
                <p className="mt-1 break-all font-mono text-lg text-brand-gold">{phone.id ?? "Not returned"}</p>
              </div>
              <div>
                <p className="text-xs uppercase tracking-wider text-brand-ivory/50">Verified name</p>
                <p className="mt-1 text-sm">{phone.verified_name ?? "—"}</p>
              </div>
              <div>
                <p className="text-xs uppercase tracking-wider text-brand-ivory/50">Quality</p>
                <p className="mt-1 text-sm">{phone.quality_rating ?? "—"}</p>
              </div>
              <div>
                <p className="text-xs uppercase tracking-wider text-brand-ivory/50">Verification</p>
                <p className="mt-1 text-sm">{phone.code_verification_status ?? "—"}</p>
              </div>
            </div>
          </section>
        ))}
      </div>
    </main>
  )
}
