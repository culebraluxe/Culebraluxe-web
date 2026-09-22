import { YewApp } from '@/components/rust-ui/yew-app'

// ---------------------------------------------------------------------------
// /faq — YEW OWNS THIS ROUTE NOW.
//
// The page is a Yew component (`rust/ui/src/yew_views/faq.rs`) driven by the MVI reducer, and the questions are the
// content store's: the items of the `faq.list` block keyed `faq`, which is the same filter `faqEntries()` applied.
//
// THE ACCORDION IS A `<details>`, NOT AN ISLAND. The live component held `useState` and animated the panel's height; an
// element that opens with no JavaScript cannot break when a script fails to arrive. The height transition is the one
// thing not reproduced — the same simplification the mobile menu makes, for the same reason — and the first question
// loads open, because that is what the component initialised to.
// ---------------------------------------------------------------------------

export default function Page() {
  return (
    <div className="min-h-screen bg-background">
      <YewApp />
    </div>
  )
}
