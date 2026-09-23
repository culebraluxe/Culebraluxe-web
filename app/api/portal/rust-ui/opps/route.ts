import { NextResponse, type NextRequest } from 'next/server'

import { withApiHandler } from '@/lib/error-capture-seam'
import {
  rustApiCreatePropertyAdmin,
  rustApiRead,
  rustApiUpdatePersonAdmin,
  rustApiUpdateProject,
  rustApiUpdatePropertyAdmin,
} from '@/lib/rust-api/client'

const PAGE_SIZE = 50

type EntityKind = 'property' | 'person' | 'project'

type WorkbenchRow = {
  id: string
  title: string
  subtitle: string | null
  status: string
  meta: string | null
}

type PropertyAdminSummary = {
  id: string
  name: string
  status: string
  location: string | null
  listPrice: string | null
  propertyType: string | null
  isActiveListing: boolean
  isPublished: boolean
  archived: boolean
  imageCount: number
  videoCount: number
}

type PropertyStellarDetails = {
  listingContractDate: string | null
  expirationDate: string | null
  listingType: string | null
  agentMlsId: string | null
  taxId: string | null
  taxYear: string | null
  annualTax: string | null
  legalDescription: string | null
  zoning: string | null
  totalAreaSqft: string | null
  heatedAreaSource: string | null
  ownershipType: string | null
  hoaDetails: string | null
  showingInstructions: string | null
  occupantType: string | null
}

type PropertyAdminRecord = PropertyAdminSummary & {
  slug: string | null
  featured: boolean
  addressLine1: string | null
  streetNumber: string | null
  streetName: string | null
  unitNumber: string | null
  city: string | null
  stateOrProvince: string | null
  neighborhood: string | null
  postalCode: string | null
  country: string | null
  isoCountryCode: string | null
  latitude: string | null
  longitude: string | null
  bedrooms: string | null
  bathrooms: string | null
  bathroomsFull: string | null
  bathroomsHalf: string | null
  squareFeet: string | null
  lotSize: string | null
  lotSizeUnits: string | null
  yearBuilt: string | null
  stories: string | null
  parkingSpaces: string | null
  shortDescription: string | null
  editorialDescription: string | null
  publicRemarks: string | null
  listingAgentName: string | null
  listingAgentEmail: string | null
  listingAgentPhone: string | null
  listingOffice: string | null
  legalOwnerName: string | null
  listingIdentifier: string | null
  registryEntry: string | null
  fincaNumber: string | null
  registrySection: string | null
  sellerPersonId: string | null
  sellerName: string | null
  documentCount: number
  createdAt: string | null
  updatedAt: string | null
  stellar: PropertyStellarDetails
}

type PropertyAdminPage = {
  rows: PropertyAdminSummary[]
  total: number
  page: number
  pageSize: number
}

type ClientAdminRow = {
  id: string
  displayName: string
  role: string
  status: string
  location: string | null
  assignedAgent: string | null
  primaryEmail: string | null
  primaryPhone: string | null
  lastInteractionLabel: string | null
  openTaskCount: number
  activeDealCount: number
  interestCount: number
}

type ClientAdminPage = {
  rows: ClientAdminRow[]
  total: number
  page: number
  pageSize: number
}

type RustPerson = {
  id: string
  display_name: string
  status: string
  archived_at: string | null
  company: string | null
}

type ClientDetail = {
  id: string
  displayName: string
  role: string
  status: string
  location?: string | null
  email?: string | null
  phone?: string | null
}

type RustProject = {
  id: string
  name: string
  owner: string | null
  status: string
  description: string
  areas: string[]
  project_type: string | null
  playbook_id: string | null
  playbook_version: number | null
  person_id: string | null
  property_id: string | null
  contract_id: string | null
  starts_at: string | null
  ends_at: string | null
  created_at: string
  updated_at: string
}

type WorkbenchPerson = {
  id: string
  displayName: string
  role: string
  status: string
  company: string | null
  location: string | null
  email: string | null
  phone: string | null
}

type WorkbenchProject = {
  id: string
  name: string
  owner: string | null
  status: string
  description: string
  areas: string[]
  projectType: string | null
  playbookId: string | null
  playbookVersion: number | null
  personId: string | null
  propertyId: string | null
  contractId: string | null
  startsAt: string | null
  endsAt: string | null
}

type WorkbenchPayload = {
  entity: EntityKind
  rows: WorkbenchRow[]
  total: number
  page: number
  pageSize: number
  selectedId: string | null
  property: PropertyAdminRecord | null
  person: WorkbenchPerson | null
  project: WorkbenchProject | null
}

type WorkbenchCommand =
  | {
      action: 'save'
      entity: EntityKind
      id: string
      fields: Record<string, string>
      search?: string
      page?: number
    }
  | {
      action: 'createProperty'
      name: string
    }

function entityKind(value: string | null): EntityKind {
  return value === 'person' || value === 'project' ? value : 'property'
}

function clean(value: string | undefined): string | null {
  const trimmed = value?.trim() ?? ''
  return trimmed ? trimmed : null
}

function bool(value: string | undefined): boolean {
  return value === 'true'
}

function projectPayload(project: RustProject): WorkbenchProject {
  return {
    id: project.id,
    name: project.name,
    owner: project.owner,
    status: project.status,
    description: project.description,
    areas: project.areas,
    projectType: project.project_type,
    playbookId: project.playbook_id,
    playbookVersion: project.playbook_version,
    personId: project.person_id,
    propertyId: project.property_id,
    contractId: project.contract_id,
    startsAt: project.starts_at,
    endsAt: project.ends_at,
  }
}

async function propertyWorkbench(
  search: string,
  pageIndex: number,
  selected: string | null,
): Promise<WorkbenchPayload> {
  const params = new URLSearchParams({
    search,
    page: String(pageIndex + 1),
    pageSize: String(PAGE_SIZE),
  })
  const page = await rustApiRead<PropertyAdminPage>(
    ('/v1/properties/admin?' + params.toString()) as `/v1/${string}`,
  )
  const selectedId =
    (selected && page.value.rows.some((row) => row.id === selected) ? selected : null) ??
    page.value.rows[0]?.id ??
    null
  const detail = selectedId
    ? await rustApiRead<PropertyAdminRecord>(
        (`/v1/properties/${encodeURIComponent(selectedId)}/admin`) as `/v1/${string}`,
      )
    : null

  return {
    entity: 'property',
    rows: page.value.rows.map((row) => ({
      id: row.id,
      title: row.name,
      subtitle: row.location,
      status: row.archived ? 'archived' : row.status,
      meta: row.listPrice,
    })),
    total: page.value.total,
    page: page.value.page,
    pageSize: page.value.pageSize,
    selectedId,
    property: detail?.value ?? null,
    person: null,
    project: null,
  }
}

async function personWorkbench(
  search: string,
  pageIndex: number,
  selected: string | null,
): Promise<WorkbenchPayload> {
  const params = new URLSearchParams({
    view: 'admin',
    search,
    page: String(pageIndex + 1),
    pageSize: String(PAGE_SIZE),
  })
  const page = await rustApiRead<ClientAdminPage>(
    ('/v1/clients?' + params.toString()) as `/v1/${string}`,
  )
  const selectedId =
    (selected && page.value.rows.some((row) => row.id === selected) ? selected : null) ??
    page.value.rows[0]?.id ??
    null

  let person: WorkbenchPerson | null = null
  if (selectedId) {
    const encoded = encodeURIComponent(selectedId)
    const [canonical, client] = await Promise.all([
      rustApiRead<RustPerson>((`/v1/people/${encoded}`) as `/v1/${string}`),
      rustApiRead<ClientDetail | null>((`/v1/clients/${encoded}`) as `/v1/${string}`),
    ])
    person = {
      id: canonical.value.id,
      displayName: canonical.value.display_name,
      role: client.value?.role ?? 'unclassified',
      status: canonical.value.status,
      company: canonical.value.company,
      location: client.value?.location ?? null,
      email: client.value?.email ?? null,
      phone: client.value?.phone ?? null,
    }
  }

  return {
    entity: 'person',
    rows: page.value.rows.map((row) => ({
      id: row.id,
      title: row.displayName,
      subtitle: row.location,
      status: row.status,
      meta: row.primaryEmail ?? row.primaryPhone,
    })),
    total: page.value.total,
    page: page.value.page,
    pageSize: page.value.pageSize,
    selectedId,
    property: null,
    person,
    project: null,
  }
}

async function projectWorkbench(
  search: string,
  pageIndex: number,
  selected: string | null,
): Promise<WorkbenchPayload> {
  const all = await rustApiRead<RustProject[]>('/v1/projects')
  const needle = search.trim().toLowerCase()
  const filtered = needle
    ? all.value.filter((project) =>
        [
          project.name,
          project.owner,
          project.status,
          project.description,
          project.project_type,
          ...project.areas,
        ]
          .filter(Boolean)
          .join(' ')
          .toLowerCase()
          .includes(needle),
      )
    : all.value

  const start = pageIndex * PAGE_SIZE
  const rows = filtered.slice(start, start + PAGE_SIZE)
  const selectedId =
    (selected && rows.some((row) => row.id === selected) ? selected : null) ??
    rows[0]?.id ??
    null
  const selectedProject = selectedId
    ? all.value.find((project) => project.id === selectedId) ?? null
    : null

  return {
    entity: 'project',
    rows: rows.map((row) => ({
      id: row.id,
      title: row.name,
      subtitle: row.project_type,
      status: row.status,
      meta: row.owner,
    })),
    total: filtered.length,
    page: pageIndex + 1,
    pageSize: PAGE_SIZE,
    selectedId,
    property: null,
    person: null,
    project: selectedProject ? projectPayload(selectedProject) : null,
  }
}

async function workbench(
  entity: EntityKind,
  search: string,
  pageIndex: number,
  selected: string | null,
): Promise<WorkbenchPayload> {
  if (entity === 'person') return personWorkbench(search, pageIndex, selected)
  if (entity === 'project') return projectWorkbench(search, pageIndex, selected)
  return propertyWorkbench(search, pageIndex, selected)
}

function propertySaveBody(fields: Record<string, string>) {
  return {
    name: fields.name ?? '',
    slug: clean(fields.slug),
    status: fields.status || 'prospect',
    featured: bool(fields.featured),
    isActiveListing: bool(fields.isActiveListing),
    isPublished: bool(fields.isPublished),
    propertyType: clean(fields.propertyType),
    listPrice: clean(fields.listPrice),
    location: clean(fields.location),
    addressLine1: clean(fields.addressLine1),
    streetNumber: clean(fields.streetNumber),
    streetName: clean(fields.streetName),
    unitNumber: clean(fields.unitNumber),
    city: clean(fields.city),
    stateOrProvince: clean(fields.stateOrProvince),
    neighborhood: clean(fields.neighborhood),
    postalCode: clean(fields.postalCode),
    country: clean(fields.country),
    isoCountryCode: clean(fields.isoCountryCode),
    latitude: clean(fields.latitude),
    longitude: clean(fields.longitude),
    bedrooms: clean(fields.bedrooms),
    bathrooms: clean(fields.bathrooms),
    bathroomsFull: clean(fields.bathroomsFull),
    bathroomsHalf: clean(fields.bathroomsHalf),
    squareFeet: clean(fields.squareFeet),
    lotSize: clean(fields.lotSize),
    lotSizeUnits: clean(fields.lotSizeUnits),
    yearBuilt: clean(fields.yearBuilt),
    stories: clean(fields.stories),
    parkingSpaces: clean(fields.parkingSpaces),
    shortDescription: clean(fields.shortDescription),
    editorialDescription: clean(fields.editorialDescription),
    publicRemarks: clean(fields.publicRemarks),
    listingAgentName: clean(fields.listingAgentName),
    listingAgentEmail: clean(fields.listingAgentEmail),
    listingAgentPhone: clean(fields.listingAgentPhone),
    listingOffice: clean(fields.listingOffice),
    legalOwnerName: clean(fields.legalOwnerName),
    listingIdentifier: clean(fields.listingIdentifier),
    registryEntry: clean(fields.registryEntry),
    fincaNumber: clean(fields.fincaNumber),
    registrySection: clean(fields.registrySection),
    sellerPersonId: clean(fields.sellerPersonId),
    archived: bool(fields.archived),
    stellar: {
      listingContractDate: clean(fields.listingContractDate),
      expirationDate: clean(fields.expirationDate),
      listingType: clean(fields.listingType),
      agentMlsId: clean(fields.agentMlsId),
      taxId: clean(fields.taxId),
      taxYear: clean(fields.taxYear),
      annualTax: clean(fields.annualTax),
      legalDescription: clean(fields.legalDescription),
      zoning: clean(fields.zoning),
      totalAreaSqft: clean(fields.totalAreaSqft),
      heatedAreaSource: clean(fields.heatedAreaSource),
      ownershipType: clean(fields.ownershipType),
      hoaDetails: clean(fields.hoaDetails),
      showingInstructions: clean(fields.showingInstructions),
      occupantType: clean(fields.occupantType),
    },
  }
}

async function GETHandler(req: NextRequest): Promise<Response> {
  const entity = entityKind(req.nextUrl.searchParams.get('entity'))
  const search = req.nextUrl.searchParams.get('search')?.trim() ?? ''
  const pageIndex = Math.max(
    0,
    Number.parseInt(req.nextUrl.searchParams.get('page') ?? '0', 10) || 0,
  )
  const selected = req.nextUrl.searchParams.get('selected')?.trim() || null
  return NextResponse.json({ ops: await workbench(entity, search, pageIndex, selected) })
}

async function POSTHandler(req: NextRequest): Promise<Response> {
  const command = (await req.json()) as WorkbenchCommand

  if (command.action === 'createProperty') {
    const created = await rustApiCreatePropertyAdmin<PropertyAdminRecord>({
      name: command.name,
    })
    return NextResponse.json({
      ops: await workbench('property', '', 0, created.value.id),
    })
  }

  if (command.action !== 'save' || !command.id?.trim()) {
    return NextResponse.json({ error: 'A workbench save requires an entity and id.' }, { status: 400 })
  }

  if (command.entity === 'property') {
    await rustApiUpdatePropertyAdmin<PropertyAdminRecord>(
      command.id,
      propertySaveBody(command.fields),
    )
  } else if (command.entity === 'person') {
    await rustApiUpdatePersonAdmin<RustPerson>(command.id, {
      displayName: command.fields.displayName ?? '',
      status: command.fields.status ?? '',
      company: clean(command.fields.company),
    })
  } else {
    await rustApiUpdateProject<RustProject>(command.id, {
      name: clean(command.fields.name),
      owner: clean(command.fields.owner),
      status: clean(command.fields.status),
      description: command.fields.description ?? '',
      areas: (command.fields.areas ?? '')
        .split(',')
        .map((value) => value.trim())
        .filter(Boolean),
      projectType: clean(command.fields.projectType),
      playbookId: clean(command.fields.playbookId),
      playbookVersion: clean(command.fields.playbookVersion)
        ? Number.parseInt(command.fields.playbookVersion, 10)
        : null,
      personId: clean(command.fields.personId),
      propertyId: clean(command.fields.propertyId),
      contractId: clean(command.fields.contractId),
    })
  }

  return NextResponse.json({
    ops: await workbench(
      command.entity,
      command.search?.trim() ?? '',
      Math.max(0, command.page ?? 0),
      command.id,
    ),
  })
}

export const GET = withApiHandler(
  { label: '/api/portal/rust-ui/opps', route: '/api/portal/rust-ui/opps' },
  GETHandler,
)

export const POST = withApiHandler(
  { label: '/api/portal/rust-ui/opps', route: '/api/portal/rust-ui/opps' },
  POSTHandler,
)
