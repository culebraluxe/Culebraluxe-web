import type { CouncilPanel, CouncilSessionDto, RecordVerdictRequest } from './types'

/**
 * The only place a model is invoked. CIL-03 ships NullModelPort.
 * CIL-05 binds DeepSeek / OpenCode. Upstream /council CLI is not a port.
 */
export interface CouncilModelPort {
  deliberate(input: {
    session: CouncilSessionDto
    panel: CouncilPanel
    brief: string
  }): Promise<Omit<RecordVerdictRequest, 'sessionId'>>
}

export class NullModelPort implements CouncilModelPort {
  async deliberate(): Promise<Omit<RecordVerdictRequest, 'sessionId'>> {
    throw new Error(
      'COUNCIL_MODEL_UNBOUND: bind a CouncilModelPort before convening a live deliberation',
    )
  }
}
