import { SiteHeader } from "@/components/site-header";
import { SiteFooter } from "@/components/site-footer";

export const metadata = {
  title: "WhatsApp | CulebraLuxe",
  description: "Official CulebraLuxe WhatsApp business contact information.",
};

export default function WhatsAppPage() {
  return (
    <>
      <SiteHeader />
      <main className="min-h-[70vh] bg-brand-navy px-6 py-20 text-brand-ivory md:px-12">
        <section className="mx-auto max-w-3xl text-center">
          <p className="mb-4 text-sm uppercase tracking-[0.3em] text-brand-gold">
            Official Business Contact
          </p>
          <h1 className="font-serif text-4xl font-medium md:text-5xl">
            CulebraLuxe WhatsApp
          </h1>
          <p className="mx-auto mt-6 max-w-xl text-base leading-7 text-brand-ivory/80">
            Contact CulebraLuxe through our official WhatsApp Business number.
          </p>

          <a
            href="https://wa.me/17876383333"
            className="mt-10 inline-flex rounded-full border border-brand-gold/60 px-8 py-4 text-lg tracking-wide text-brand-ivory transition hover:border-brand-gold hover:text-brand-gold"
          >
            +1 (787) 638-3333
          </a>
        </section>
      </main>
      <SiteFooter />
    </>
  );
}
