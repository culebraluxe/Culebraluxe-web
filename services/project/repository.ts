import type {
  CompleteProjectRequest,
  CreateProjectRequest,
  Project,
  UpdateProjectRequest,
} from './types'

/** Persistence boundary for the Project domain (parent of WBS children). */
export interface ProjectRepository {
  get(id: string): Promise<Project | null>
  list(): Promise<Project[]>
  create(request: CreateProjectRequest): Promise<Project>
  update(request: UpdateProjectRequest): Promise<Project>
  complete(request: CompleteProjectRequest): Promise<Project>
}
