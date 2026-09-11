import 'server-only'

import { coreEntitlements, coreServices } from '@/lib/service-runtime'

/**
 * The kernel has ONE composition — `lib/service-runtime.ts`. These aliases keep
 * the forms/hydration call sites working; new code should import from
 * `lib/service-runtime.ts` directly.
 */

/** Explicit entitlement port for the service kernel. */
export const formEntitlements = coreEntitlements

/** The one production kernel composition: all six domains, one place. */
export const formCoreServices = coreServices

/** Showing is a first-class composed domain now (kept as a stable alias). */
export const formShowingService = coreServices.showing
