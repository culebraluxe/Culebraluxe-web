// ---------------------------------------------------------------------------
// SERVICE KERNEL — public surface.
//
// FREEZE: new business writes for Person / Firm / Property / Contract /
// Showing / Security go through service envelopes ONLY (BaseService + owning
// repository port). Do not add drive-by writers under `lib/commands/*` or raw
// `db/*` for these domains.
// ---------------------------------------------------------------------------
export * from './core'
export * from './person'
export * from './firm'
export * from './property'
export * from './contract'
export * from './wbs'
export {
  composeCoreServices,
  type CoreServiceComposition,
  type CoreServiceRepositories,
} from './composition'
