import { forwardRef, type FieldsetHTMLAttributes, type InputHTMLAttributes, type LabelHTMLAttributes, type ReactNode, type SelectHTMLAttributes, type TextareaHTMLAttributes } from "react"

import { cn } from "@/lib/utils"

export const portalControlClass =
  "min-h-11 w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white px-3 text-sm font-light text-[var(--portal-text)] outline-none transition placeholder:text-black/35 hover:border-[var(--portal-blue-gray)]/60 focus:border-[var(--portal-navy-soft)] focus:ring-1 focus:ring-[var(--portal-gold)]/35 disabled:cursor-not-allowed disabled:bg-black/[0.03] disabled:text-black/35 aria-[invalid=true]:border-[var(--portal-archive)] aria-[invalid=true]:ring-1 aria-[invalid=true]:ring-[var(--portal-danger)]/25"

export function PortalField({
  children,
  className,
}: {
  children: ReactNode
  className?: string
}) {
  return <div className={cn("space-y-2", className)}>{children}</div>
}

export function PortalFieldLabel({
  className,
  ...props
}: LabelHTMLAttributes<HTMLLabelElement>) {
  return (
    <label
      className={cn(
        "block text-[10px] font-light uppercase tracking-[0.18em] text-black/45",
        className,
      )}
      {...props}
    />
  )
}

export function PortalFieldDescription({
  children,
  className,
  id,
}: {
  children: ReactNode
  className?: string
  id?: string
}) {
  return (
    <p id={id} className={cn("text-xs font-light leading-5 text-black/45", className)}>
      {children}
    </p>
  )
}

export function PortalFieldError({
  children,
  className,
  id,
}: {
  children: ReactNode
  className?: string
  id?: string
}) {
  return (
    <p
      id={id}
      role="alert"
      className={cn("text-xs font-light leading-5 text-[var(--portal-archive)]", className)}
    >
      {children}
    </p>
  )
}

export function PortalFieldset({
  className,
  ...props
}: FieldsetHTMLAttributes<HTMLFieldSetElement>) {
  return <fieldset className={cn("space-y-4", className)} {...props} />
}

export function PortalLegend({
  children,
  className,
}: {
  children: ReactNode
  className?: string
}) {
  return (
    <legend className={cn("font-serif text-xl font-light text-[var(--portal-navy)]", className)}>
      {children}
    </legend>
  )
}

export const PortalInput = forwardRef<HTMLInputElement, InputHTMLAttributes<HTMLInputElement>>(
  function PortalInput({ className, ...props }, ref) {
    return <input ref={ref} className={cn(portalControlClass, className)} {...props} />
  },
)

export const PortalSelect = forwardRef<HTMLSelectElement, SelectHTMLAttributes<HTMLSelectElement>>(
  function PortalSelect({ className, ...props }, ref) {
    return (
      <select
        ref={ref}
        className={cn(portalControlClass, "appearance-none", className)}
        {...props}
      />
    )
  },
)

export const PortalTextarea = forwardRef<
  HTMLTextAreaElement,
  TextareaHTMLAttributes<HTMLTextAreaElement>
>(function PortalTextarea({ className, ...props }, ref) {
  return (
    <textarea
      ref={ref}
      className={cn(portalControlClass, "min-h-24 resize-y py-2", className)}
      {...props}
    />
  )
})
