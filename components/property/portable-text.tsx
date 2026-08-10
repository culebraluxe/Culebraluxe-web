import {
  PortableText as BasePortableText,
  type PortableTextComponents,
} from '@portabletext/react'

const components: PortableTextComponents = {
  block: {
    normal: ({ children }) => (
      <p className="mb-5 text-pretty text-[15px] font-light leading-relaxed text-foreground/80 last:mb-0">
        {children}
      </p>
    ),
    h2: ({ children }) => (
      <h3 className="mb-4 mt-8 font-serif text-2xl font-light leading-tight text-foreground first:mt-0">
        {children}
      </h3>
    ),
    h3: ({ children }) => (
      <h4 className="mb-3 mt-6 font-serif text-xl font-light leading-tight text-foreground first:mt-0">
        {children}
      </h4>
    ),
    blockquote: ({ children }) => (
      <blockquote className="my-6 border-l border-accent pl-6 font-serif text-xl font-light italic leading-relaxed text-foreground/75">
        {children}
      </blockquote>
    ),
  },
  list: {
    bullet: ({ children }) => (
      <ul className="mb-5 flex flex-col gap-2 text-[15px] font-light leading-relaxed text-foreground/80">
        {children}
      </ul>
    ),
    number: ({ children }) => (
      <ol className="mb-5 flex list-decimal flex-col gap-2 pl-5 text-[15px] font-light leading-relaxed text-foreground/80">
        {children}
      </ol>
    ),
  },
  listItem: {
    bullet: ({ children }) => (
      <li className="flex gap-3">
        <span
          aria-hidden
          className="mt-2 h-px w-4 flex-none bg-accent"
        />
        <span>{children}</span>
      </li>
    ),
  },
  marks: {
    strong: ({ children }) => (
      <strong className="font-medium text-foreground">{children}</strong>
    ),
    em: ({ children }) => <em className="italic">{children}</em>,
    link: ({ children, value }) => (
      <a
        href={value?.href}
        rel="noopener noreferrer"
        className="underline decoration-accent/40 underline-offset-4 transition-colors hover:decoration-accent"
      >
        {children}
      </a>
    ),
  },
}

export function PropertyPortableText({ value }: { value: unknown }) {
  if (!value || (Array.isArray(value) && value.length === 0)) return null
  // @portabletext/react expects a typed array; the shape is validated by Sanity.
  return <BasePortableText value={value as never} components={components} />
}
