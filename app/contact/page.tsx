import { YewApp } from '@/components/rust-ui/yew-app'

// ---------------------------------------------------------------------------
// /contact — YEW OWNS THIS ROUTE NOW.
//
// The page is a Yew component (`rust/ui/src/yew_views/contact.rs`) driven by the MVI reducer: the hero from the content
// store, the section's own copy, and the office and email it carries — the email as the `mailto:` link it already was.
//
// THE FORM IS NOT HERE, AND THAT IS NOT AN OMISSION. Its submission state was React state and its submit called a server
// action whose request never completed on the live site. It comes back when there is an endpoint a plain
// `<form method="post">` can target — then the fields, the interest choice and the sent/failed states are Rust markup,
// and the outcome travels in the URL. A form that appears to send and does not is worse than a page that says how to
// reach someone.
// ---------------------------------------------------------------------------

export default function Page() {
  return (
    <div className="min-h-screen bg-background">
      <YewApp />
    </div>
  )
}
