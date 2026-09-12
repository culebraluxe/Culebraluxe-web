export type RouteIntent = 'SOLO' | 'SMITH' | 'SPLIT'
export type UniverseBucket = 'open' | 'backlog' | 'closed' | 'next'
export type LineStation = 'ready' | 'running' | 'hold' | 'done'

export type LineStory = {
  id: string
  title: string
  workstream: string
  status: string
  bucket: UniverseBucket
  intent?: string
  waitsFor: string[]
  completion: number
  onBench: boolean
  routeIntent: RouteIntent
  seamsNamed: boolean
  depsClear: boolean
  allowedScope: string
  prohibitedScope: string
  acceptance: string[]
  line?: {
    station: LineStation
    node?: string
    attempt?: number
    widgets?: string
    sha?: string
    qa?: string
    reason?: string
    instanceId?: string
    children?: { label: string; sha: string }[]
  }
}

export type EngineeringLineModel = {
  freshnessUtc: string
  roles: string[]
  stories: LineStory[]
}
