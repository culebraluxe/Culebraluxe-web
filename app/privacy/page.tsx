import type { Metadata } from 'next'

import { SiteFooter } from '@/components/site-footer'
import { SiteHeader } from '@/components/site-header'

export const metadata: Metadata = {
  title: 'Privacy Policy — CulebraLuxe',
  description: 'CulebraLuxe privacy policy for website, account, messaging, and WhatsApp integrations.',
}

const LAST_UPDATED = 'September 1, 2026'

export default function PrivacyPolicyPage() {
  return (
    <>
      <SiteHeader />

      <main className="px-6 py-20 md:px-12 md:py-28">
        <div className="mx-auto max-w-4xl">
          <p className="text-xs font-light uppercase tracking-[0.28em] text-accent">CulebraLuxe LLC</p>
          <h1 className="mt-4 font-serif text-4xl font-light leading-tight text-foreground md:text-5xl">
            Privacy Policy
          </h1>
          <p className="mt-4 text-sm font-light text-muted-foreground">Last updated: {LAST_UPDATED}</p>

          <div className="mt-12 space-y-10 text-sm font-light leading-7 text-muted-foreground">
            <section>
              <h2 className="font-serif text-2xl font-light text-foreground">Overview</h2>
              <p className="mt-4">
                CulebraLuxe LLC respects your privacy. This Privacy Policy explains how we collect, use,
                store, and protect information when you use our website, communicate with us, or interact
                with services and integrations operated by CulebraLuxe, including WhatsApp and Meta
                services.
              </p>
            </section>

            <section>
              <h2 className="font-serif text-2xl font-light text-foreground">Information we collect</h2>
              <p className="mt-4">
                We may collect information you provide directly to us, such as your name, email address,
                phone number, property information, communication preferences, and other information you
                choose to provide when contacting CulebraLuxe or using our services.
              </p>
              <p className="mt-4">
                When you communicate with CulebraLuxe through WhatsApp or other messaging services, we may
                receive information associated with those communications, including identifiers, phone
                numbers, timestamps, message status information, and message content when required to
                provide the requested communication service.
              </p>
            </section>

            <section>
              <h2 className="font-serif text-2xl font-light text-foreground">Meta and WhatsApp integrations</h2>
              <p className="mt-4">
                CulebraLuxe may use Meta Platforms, Inc. services, including Facebook Login for Business,
                the WhatsApp Business Platform, and WhatsApp Business App coexistence features. When you
                authorize or interact with these services, Meta may provide CulebraLuxe with information
                necessary to operate the integration, such as business account identifiers, WhatsApp
                Business Account information, phone number identifiers, access authorization information,
                webhook events, and messaging data associated with CulebraLuxe communications.
              </p>
              <p className="mt-4">
                We use this information only to operate CulebraLuxe business communications, maintain our
                customer and relationship records, provide requested services, troubleshoot integrations,
                and comply with applicable legal or platform requirements.
              </p>
            </section>

            <section>
              <h2 className="font-serif text-2xl font-light text-foreground">How we use information</h2>
              <p className="mt-4">We may use collected information to:</p>
              <ul className="mt-3 list-disc space-y-2 pl-6">
                <li>respond to inquiries and communicate with clients and prospective clients;</li>
                <li>provide real estate brokerage and related services;</li>
                <li>maintain client, property, transaction, and relationship records;</li>
                <li>operate and improve our website, internal systems, and messaging integrations;</li>
                <li>protect against fraud, misuse, security incidents, or unauthorized access; and</li>
                <li>comply with legal, regulatory, contractual, and platform obligations.</li>
              </ul>
            </section>

            <section>
              <h2 className="font-serif text-2xl font-light text-foreground">Sharing of information</h2>
              <p className="mt-4">
                We do not sell personal information. We may share information with service providers and
                technology platforms only as needed to operate CulebraLuxe services, including hosting,
                communications, document, authentication, and messaging services. We may also disclose
                information when required by law or when reasonably necessary to protect CulebraLuxe,
                our clients, or others.
              </p>
            </section>

            <section>
              <h2 className="font-serif text-2xl font-light text-foreground">Data retention and security</h2>
              <p className="mt-4">
                We retain information only for as long as reasonably necessary for the purposes described
                in this policy, for legitimate business and recordkeeping needs, and as required by law.
                We use reasonable administrative, technical, and organizational safeguards designed to
                protect information from unauthorized access, loss, misuse, or disclosure.
              </p>
            </section>

            <section>
              <h2 className="font-serif text-2xl font-light text-foreground">Your choices and requests</h2>
              <p className="mt-4">
                You may contact us to ask about personal information associated with you, request a
                correction, or request deletion where applicable. Some information may be retained when
                required for legal, regulatory, transaction-record, security, or legitimate business
                purposes.
              </p>
            </section>

            <section>
              <h2 className="font-serif text-2xl font-light text-foreground">Third-party services</h2>
              <p className="mt-4">
                Our services may interact with third-party platforms such as Meta and WhatsApp. Those
                services operate under their own privacy policies and terms. CulebraLuxe is not responsible
                for the privacy practices of third-party services except for our own collection and use of
                information received through them.
              </p>
            </section>

            <section>
              <h2 className="font-serif text-2xl font-light text-foreground">Contact us</h2>
              <p className="mt-4">
                Questions or privacy requests may be sent to CulebraLuxe LLC through our public contact page
                at https://www.culebraluxe.com/contact.
              </p>
            </section>
          </div>
        </div>
      </main>

      <SiteFooter />
    </>
  )
}
