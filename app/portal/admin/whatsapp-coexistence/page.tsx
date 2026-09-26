"use client"

// ---------------------------------------------------------------------------
// LIFELINE — DO NOT CONVERT, DO NOT "IMPROVE".
//
// The WhatsApp Business coexistence activation (Meta Embedded Signup). It took four weeks to get messages flowing,
// and they flow. This is the exact code that did it: restored byte-for-byte from 706329da (2026-09-15, "finish
// coexistence after embedded signup"), after the Yew conversion (cb5000a2) replaced it with a body that had lost the
// launcher. It is kept so there is a way back if the activation ever has to be redone.
//
// Owner's instruction (2026-09-26): keep the original as-is; any fix here is done deliberately, by the owner, when
// needed. It is registered as a page Next renders (`Kind::External` in rust/ui/src/app/registry.rs), outside the
// Rust/Yew cutover. Opening it loads the Meta SDK; it changes nothing until someone completes the Meta flow.
// ---------------------------------------------------------------------------

import { useEffect, useState } from "react"

declare global {
  interface Window {
    FB?: {
      init: (options: {
        appId: string
        cookie?: boolean
        xfbml?: boolean
        version: string
      }) => void
      login: (
        callback: (response: {
          authResponse?: { code?: string }
          status?: string
        }) => void,
        options: Record<string, unknown>,
      ) => void
    }
    fbAsyncInit?: () => void
  }
}

const META_APP_ID = "1573618894304413"
const GRAPH_VERSION = "v26.0"
const META_CONFIGURATION_ID = "1416075310402629"

type SessionEvent = {
  type?: string
  event?: string
  version?: number
  data?: {
    waba_id?: string
    phone_number_id?: string
    [key: string]: unknown
  }
  [key: string]: unknown
}

type CompletionResult = {
  ok: boolean
  error?: string
  warning?: string
  subscribed?: boolean
  wabaId?: string
  phoneNumberId?: string
  phoneStatus?: {
    id?: string
    displayPhoneNumber?: string | null
    isOnBusinessApp?: boolean | null
    platformType?: string | null
    status?: string | null
    codeVerificationStatus?: string | null
  } | null
}

export default function WhatsAppCoexistencePage() {
  const [sdkReady, setSdkReady] = useState(false)
  const [safetyConfirmed, setSafetyConfirmed] = useState(false)
  const [status, setStatus] = useState("Loading Meta SDK…")
  const [sessionEvent, setSessionEvent] = useState<SessionEvent | null>(null)
  const [authorizationCode, setAuthorizationCode] = useState<string | null>(null)
  const [completionStarted, setCompletionStarted] = useState(false)
  const [completionResult, setCompletionResult] = useState<CompletionResult | null>(null)

  useEffect(() => {
    const handleMessage = (event: MessageEvent) => {
      if (event.origin !== "https://www.facebook.com" && event.origin !== "https://web.facebook.com") {
        return
      }

      let payload: unknown = event.data
      if (typeof payload === "string") {
        try {
          payload = JSON.parse(payload)
        } catch {
          return
        }
      }

      if (!payload || typeof payload !== "object") return
      const candidate = payload as SessionEvent
      if (candidate.type !== "WA_EMBEDDED_SIGNUP") return

      setSessionEvent(candidate)
      if (candidate.event === "FINISH_WHATSAPP_BUSINESS_APP_ONBOARDING") {
        setStatus("Coexistence onboarding finished in Meta. Waiting for the authorization code…")
      } else {
        setStatus(`Meta Embedded Signup event: ${candidate.event ?? "unknown"}`)
      }
    }

    window.addEventListener("message", handleMessage)

    window.fbAsyncInit = () => {
      window.FB?.init({
        appId: META_APP_ID,
        cookie: true,
        xfbml: false,
        version: GRAPH_VERSION,
      })
      setSdkReady(true)
      setStatus("Meta SDK ready. Confirm the coexistence safety check to continue.")
    }

    if (document.getElementById("facebook-jssdk")) {
      if (window.FB) window.fbAsyncInit()
    } else {
      const script = document.createElement("script")
      script.id = "facebook-jssdk"
      script.async = true
      script.defer = true
      script.crossOrigin = "anonymous"
      script.src = "https://connect.facebook.net/en_US/sdk.js"
      document.body.appendChild(script)
    }

    return () => {
      window.removeEventListener("message", handleMessage)
    }
  }, [])

  // Meta returns the session asset IDs and the one-time authorization code over
  // two independent channels. Complete the transaction only after BOTH have
  // arrived. This is the server-side step the old launcher was missing.
  useEffect(() => {
    if (completionStarted || !authorizationCode) return
    if (sessionEvent?.event !== "FINISH_WHATSAPP_BUSINESS_APP_ONBOARDING") return

    const wabaId = sessionEvent.data?.waba_id?.trim()
    if (!wabaId) return

    setCompletionStarted(true)
    setStatus("Meta signup finished. Completing Coexistence on the CulebraLuxe server…")

    void (async () => {
      try {
        const response = await fetch("/api/integrations/whatsapp/coexistence/complete", {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({
            code: authorizationCode,
            wabaId,
            phoneNumberId: sessionEvent.data?.phone_number_id ?? null,
          }),
        })
        const payload = await response.json().catch(() => null) as CompletionResult | null

        if (!response.ok || !payload?.ok) {
          setCompletionResult(payload)
          setStatus(`Server completion failed: ${payload?.error ?? `HTTP ${response.status}`}`)
          return
        }

        setCompletionResult(payload)
        const phone = payload.phoneStatus
        if (phone?.isOnBusinessApp === true && phone.platformType === "CLOUD_API") {
          setStatus("Coexistence complete. Meta now reports the existing Business App number connected to Cloud API.")
          return
        }

        if (phone) {
          setStatus(
            `Server completion succeeded. Meta phone status: is_on_biz_app=${String(phone.isOnBusinessApp)}, platform_type=${phone.platformType ?? "unknown"}.`,
          )
          return
        }

        setStatus(payload.warning ?? "Server completion succeeded. Recheck Meta phone status in a moment.")
      } catch (error) {
        setCompletionResult({ ok: false, error: error instanceof Error ? error.message : "Network error" })
        setStatus("Server completion failed before Meta could be confirmed.")
      }
    })()
  }, [authorizationCode, completionStarted, sessionEvent])

  const canLaunch = sdkReady && safetyConfirmed

  function launchCoexistence() {
    if (!window.FB || !sdkReady || !safetyConfirmed) return

    setStatus("Opening Meta Coexistence Embedded Signup…")
    setSessionEvent(null)
    setAuthorizationCode(null)
    setCompletionStarted(false)
    setCompletionResult(null)

    // Important: FB.login must run synchronously from this click handler or browsers can block the popup.
    window.FB.login(
      (response) => {
        const code = response.authResponse?.code?.trim()
        if (code) {
          // Never render or log the one-time code. Keep it only in memory long
          // enough to hand it to our authenticated server completion endpoint.
          setAuthorizationCode(code)
          setStatus((current) =>
            current.startsWith("Coexistence onboarding finished in Meta.")
              ? "Meta signup finished. Completing Coexistence on the CulebraLuxe server…"
              : "Meta returned the Embedded Signup authorization code. Waiting for the finish event…",
          )
          return
        }

        setStatus(response.status ? `Meta login status: ${response.status}` : "Meta signup was closed or cancelled.")
      },
      {
        config_id: META_CONFIGURATION_ID,
        response_type: "code",
        override_default_response_type: true,
        extras: {
          setup: {},
          featureType: "whatsapp_business_app_onboarding",
          sessionInfoVersion: "3",
        },
      },
    )
  }

  return (
    <main className="min-h-screen bg-brand-navy px-6 py-12 text-brand-ivory">
      <div className="mx-auto max-w-3xl space-y-8">
        <div>
          <p className="text-xs uppercase tracking-[0.28em] text-brand-gold">Private diagnostic</p>
          <h1 className="mt-3 font-serif text-3xl">WhatsApp Coexistence Launcher</h1>
          <p className="mt-3 text-sm text-brand-ivory/70">
            Launches Meta Embedded Signup in WhatsApp Business App coexistence mode, then completes the
            returned authorization transaction server-side. It does not register, migrate, disconnect,
            replace, or delete the existing phone number.
          </p>
        </div>

        <section className="rounded-2xl border border-brand-gold/25 bg-white/5 p-6 space-y-5">
          <div className="grid gap-4 sm:grid-cols-2">
            <div>
              <p className="text-xs uppercase tracking-wider text-brand-ivory/50">Meta App ID</p>
              <p className="mt-1 font-mono text-sm">{META_APP_ID}</p>
            </div>
            <div>
              <p className="text-xs uppercase tracking-wider text-brand-ivory/50">Launch mode</p>
              <p className="mt-1 text-sm text-brand-gold">WhatsApp Business App coexistence</p>
            </div>
          </div>

          <div>
            <p className="text-xs uppercase tracking-wider text-brand-ivory/50">
              Embedded Signup Configuration ID
            </p>
            <p className="mt-1 font-mono text-sm">{META_CONFIGURATION_ID}</p>
          </div>

          <label className="flex items-start gap-3 rounded-xl border border-amber-400/30 bg-amber-400/10 p-4">
            <input
              type="checkbox"
              checked={safetyConfirmed}
              onChange={(event) => setSafetyConfirmed(event.target.checked)}
              className="mt-1 size-4 accent-[var(--color-brand-gold)]"
            />
            <span className="text-sm leading-6 text-brand-ivory/80">
              I am connecting the existing WhatsApp Business App number through Coexistence. I will stop
              if Meta shows migration, unregister, disconnect, replace, or delete language.
            </span>
          </label>

          <button
            type="button"
            disabled={!canLaunch || completionStarted}
            onClick={launchCoexistence}
            className="rounded-xl border border-brand-gold/40 bg-brand-gold/10 px-5 py-3 text-sm font-medium text-brand-gold disabled:cursor-not-allowed disabled:opacity-40"
          >
            Connect Existing WhatsApp Business Number
          </button>

          <div className="rounded-xl border border-white/10 bg-black/15 p-4">
            <p className="text-xs uppercase tracking-wider text-brand-ivory/50">Status</p>
            <p className="mt-2 text-sm">{status}</p>
          </div>
        </section>

        <section className="rounded-2xl border border-brand-gold/25 bg-white/5 p-6">
          <h2 className="font-medium">Coexistence transaction</h2>
          <dl className="mt-4 grid gap-3 text-sm sm:grid-cols-2">
            <div>
              <dt className="text-brand-ivory/50">featureType</dt>
              <dd className="mt-1 font-mono text-xs text-brand-gold">whatsapp_business_app_onboarding</dd>
            </div>
            <div>
              <dt className="text-brand-ivory/50">sessionInfoVersion</dt>
              <dd className="mt-1 font-mono text-xs">3</dd>
            </div>
            <div>
              <dt className="text-brand-ivory/50">Authorization code returned</dt>
              <dd className="mt-1 text-xs">{authorizationCode ? "Yes" : "Not yet"}</dd>
            </div>
            <div>
              <dt className="text-brand-ivory/50">Server completion</dt>
              <dd className="mt-1 text-xs">
                {completionResult?.ok ? "Complete" : completionStarted ? "Running" : "Not started"}
              </dd>
            </div>
          </dl>
        </section>

        {completionResult?.phoneStatus ? (
          <section className="rounded-2xl border border-brand-gold/25 bg-white/5 p-6">
            <h2 className="font-medium">Meta phone status after completion</h2>
            <dl className="mt-4 grid gap-3 text-sm sm:grid-cols-2">
              <div>
                <dt className="text-brand-ivory/50">Business App</dt>
                <dd className="mt-1 font-mono text-xs">{String(completionResult.phoneStatus.isOnBusinessApp)}</dd>
              </div>
              <div>
                <dt className="text-brand-ivory/50">Platform</dt>
                <dd className="mt-1 font-mono text-xs">{completionResult.phoneStatus.platformType ?? "unknown"}</dd>
              </div>
              <div>
                <dt className="text-brand-ivory/50">Number</dt>
                <dd className="mt-1 font-mono text-xs">{completionResult.phoneStatus.displayPhoneNumber ?? "not returned"}</dd>
              </div>
              <div>
                <dt className="text-brand-ivory/50">Status</dt>
                <dd className="mt-1 font-mono text-xs">{completionResult.phoneStatus.status ?? "not returned"}</dd>
              </div>
            </dl>
          </section>
        ) : null}

        {sessionEvent ? (
          <section className="rounded-2xl border border-brand-gold/25 bg-white/5 p-6">
            <h2 className="font-medium">Latest Meta session event</h2>
            <pre className="mt-4 overflow-x-auto whitespace-pre-wrap break-words rounded-xl bg-black/20 p-4 text-xs text-brand-ivory/80">
              {JSON.stringify(sessionEvent, null, 2)}
            </pre>
          </section>
        ) : null}

        <section className="rounded-2xl border border-amber-400/30 bg-amber-400/10 p-6">
          <p className="text-sm text-brand-ivory/80">
            The server completion exchanges Meta&apos;s one-time code and subscribes the existing WABA. It
            deliberately contains no phone registration endpoint. Do not use Meta&apos;s ordinary phone
            migration/disconnect flow for the existing Business App number.
          </p>
        </section>
      </div>
    </main>
  )
}
