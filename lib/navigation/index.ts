// ---------------------------------------------------------------------------
// UI-01 — Operating-surface navigation registry: public surface.
//
// The single answer to "which surface owns this route?" and "what navigation
// belongs under this surface?" — used by the application shell and tests.
// ---------------------------------------------------------------------------

export {
  OPERATING_SURFACE_ORDER,
  OPERATING_SURFACES,
  surfaceForPathname,
  navigationForSurface,
  surfaceHome,
  surfaceDefinition,
  isOperatingSurface,
} from './registry'
export type {
  OperatingSurface,
  OperatingSurfaceDefinition,
  SurfaceNavItem,
} from './types'
