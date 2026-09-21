import type { SaveShowingReportRequest, ShowingDto } from '@/legacy/services/showing/types'

export interface ShowingRepository {
  get(showingId: string): Promise<ShowingDto | null>
  saveReport(request: SaveShowingReportRequest): Promise<ShowingDto>
}
