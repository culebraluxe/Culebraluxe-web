import type { SaveShowingReportRequest, ShowingDto } from './types'

export interface ShowingRepository {
  get(showingId: string): Promise<ShowingDto | null>
  saveReport(request: SaveShowingReportRequest): Promise<ShowingDto>
}
