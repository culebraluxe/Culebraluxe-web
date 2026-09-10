import type { ProjectsWorkspaceData } from "./model"

export type ProjectsWorkspaceLoadOptions = {
  signal?: AbortSignal
}

export interface ProjectsWorkspaceSource {
  load(options?: ProjectsWorkspaceLoadOptions): Promise<ProjectsWorkspaceData>
}

function cloneData(data: ProjectsWorkspaceData): ProjectsWorkspaceData {
  return {
    domains: data.domains.map((domain) => ({ ...domain })),
    ...(data.loadState ? { loadState: { ...data.loadState } } : {}),
    poles: data.poles.map((pole) => ({
      ...pole,
      projects: pole.projects.map((project) => ({
        ...project,
        workNodes: project.workNodes.map((node) => ({
          ...node,
          children: node.children?.map((child) => ({ ...child })),
        })),
      })),
    })),
  }
}

/** DB-free adapter for isolated MVI view/controller work. */
export class InMemoryProjectsWorkspaceSource implements ProjectsWorkspaceSource {
  constructor(private readonly data: ProjectsWorkspaceData = PROJECTS_WORKSPACE_FIXTURE) {}

  async load(): Promise<ProjectsWorkspaceData> {
    return cloneData(this.data)
  }
}

export const PROJECTS_WORKSPACE_FIXTURE: ProjectsWorkspaceData = {
  domains: [
    { key: "properties", label: "Properties", shortLabel: "Property" },
    { key: "people", label: "People", shortLabel: "People" },
    { key: "deals", label: "Deals", shortLabel: "Deals" },
    { key: "firm", label: "Firm", shortLabel: "Firm" },
    { key: "marketing", label: "Marketing", shortLabel: "Marketing" },
    { key: "accounting", label: "Accounting", shortLabel: "Accounting" },
  ],
  poles: [
    {
      id: "sea-to-soul",
      domain: "properties",
      label: "Sea to Soul",
      subtitle: "Playa Sardinas II · Jessica Iverson",
      progress: 67,
      statusLabel: "Active listing",
      projects: [
        {
          id: "listing-onboarding",
          title: "Listing Onboarding",
          kind: "LISTING",
          status: "active",
          progress: 67,
          phaseLabel: "Agreement",
          nextAction: "Obtain seller signature",
          nextActionDetail: "Send the final LISTING-01 agreement for e-signature to Jessica Iverson.",
          blocker: "Waiting on signed listing agreement. All other current items are on track.",
          workNodes: [
            {
              id: "parties",
              title: "Clients / Parties",
              type: "group",
              status: "complete",
              dueLabel: "Sep 3",
              owner: "Lisa Penfield",
              note: "Seller identity and contact details confirmed.",
              inspector: {
                summary: "Seller identity and contact confirmed.",
                relatedItems: [
                  { label: "Jessica Iverson", caption: "Seller" },
                  { label: "Sea to Soul", caption: "Property" },
                  { label: "Contact / identity", caption: "Confirmed" },
                ],
              },
              actions: ["Open Client", "View Client Profile"],
            },
            {
              id: "property",
              title: "Property",
              type: "group",
              status: "complete",
              dueLabel: "Sep 4",
              owner: "Lisa Penfield",
              note: "Property and ownership context confirmed.",
              inspector: {
                summary: "Property and ownership context confirmed.",
                relatedItems: [
                  { label: "Sea to Soul", caption: "Property" },
                  { label: "Playa Sardinas II", caption: "Location" },
                  { label: "Jessica Iverson", caption: "Seller / owner" },
                ],
              },
              actions: ["Open Property", "View Property Context"],
            },
            {
              id: "listing-agreement",
              title: "Listing Agreement",
              type: "contract",
              status: "waiting",
              dueLabel: "Sep 10",
              owner: "Lisa",
              note: "LISTING-01 v4 prepared; waiting for seller execution.",
              inspector: {
                summary: "Agreement ready for execution; awaiting the seller gate.",
                relatedItems: [
                  { label: "LISTING-01 v4", caption: "Contract" },
                  { label: "Jessica Iverson", caption: "Seller" },
                  { label: "Sea to Soul", caption: "Property" },
                  { label: "Draft Listing Agreement.pdf", caption: "Document" },
                ],
              },
              actions: ["Open Contract", "View Document", "View Activity"],
              children: [
                {
                  id: "seller-signature",
                  title: "Seller Signature",
                  type: "approval",
                  status: "waiting",
                  dueLabel: "Tomorrow",
                  owner: "Lisa Penfield",
                  note: "Jessica Iverson signature is the current gate.",
                  inspector: {
                    summary: "Jessica Iverson signature is the current gate.",
                    relatedItems: [
                      { label: "Listing Agreement", caption: "LISTING-01 v4" },
                      { label: "Jessica Iverson", caption: "Seller / Client" },
                      { label: "Sea to Soul", caption: "Property" },
                      { label: "Draft Listing Agreement.pdf", caption: "Document" },
                    ],
                  },
                  actions: ["Open Contract", "Send Reminder", "View Activity"],
                },
              ],
            },
            {
              id: "media",
              title: "Media",
              type: "media",
              status: "in-progress",
              dueLabel: "Sep 12",
              owner: "Lisa Penfield",
              note: "Photography scheduled; cabinet intake open.",
              inspector: {
                summary: "Photography scheduled; cabinet intake open.",
                relatedItems: [
                  { label: "Photography brief", caption: "Scheduled" },
                  { label: "Cabinet intake", caption: "Open" },
                  { label: "Sea to Soul", caption: "Property" },
                ],
              },
              actions: ["Open Media", "View Cabinet", "View Activity"],
            },
            {
              id: "marketing",
              title: "Marketing",
              type: "workflow",
              status: "not-started",
              dueLabel: "Sep 14",
              owner: "Lisa",
              note: "Launch waits on agreement and first media package.",
              inspector: {
                summary: "Launch waits on the signed agreement and first media package.",
                relatedItems: [
                  { label: "Listing Agreement", caption: "Gate" },
                  { label: "Media package", caption: "Dependency" },
                ],
              },
              actions: ["Open Marketing", "View Activity"],
            },
            {
              id: "accounting",
              title: "Accounting",
              type: "accounting",
              status: "not-started",
              dueLabel: "Sep 15",
              owner: "Lisa",
              note: "Commission schedule and receivable setup.",
              inspector: {
                summary: "Commission schedule and receivable setup for the listing.",
                relatedItems: [
                  { label: "Commission schedule", caption: "Setup" },
                  { label: "Receivables", caption: "Setup" },
                ],
              },
              actions: ["Open Accounting", "View Activity"],
            },
          ],
        },
        {
          id: "marketing-launch",
          title: "Marketing Launch",
          kind: "MARKETING",
          status: "active",
          progress: 20,
          phaseLabel: "Prepare",
          nextAction: "Complete first media package",
          nextActionDetail: "Select hero photography and approve the launch copy package.",
          workNodes: [
            { id: "launch-media", title: "Media package", type: "media", status: "in-progress", owner: "Lisa" },
            { id: "launch-copy", title: "Listing copy", type: "task", status: "not-started", owner: "Lisa" },
            { id: "launch-channels", title: "Channel release", type: "workflow", status: "not-started", owner: "Lisa" },
          ],
        },
        {
          id: "closing",
          title: "Closing",
          kind: "CLOSING",
          status: "planning",
          progress: 0,
          phaseLabel: "Not started",
          workNodes: [
            { id: "closing-ready", title: "Closing readiness", type: "milestone", status: "not-started" },
          ],
        },
      ],
    },
    {
      id: "ocean-view",
      domain: "properties",
      label: "Ocean View",
      subtitle: "Bahía Ridge · Active seller",
      progress: 18,
      statusLabel: "Preparing",
      projects: [
        {
          id: "ocean-listing",
          title: "Listing Onboarding",
          kind: "LISTING",
          status: "active",
          progress: 18,
          phaseLabel: "Property",
          nextAction: "Confirm legal owner",
          workNodes: [
            { id: "ocean-parties", title: "Clients / Parties", type: "group", status: "complete" },
            { id: "ocean-property", title: "Property", type: "group", status: "in-progress" },
            { id: "ocean-agreement", title: "Listing Agreement", type: "contract", status: "not-started" },
          ],
        },
      ],
    },
    {
      id: "pine-street",
      domain: "properties",
      label: "Pine Street",
      subtitle: "Town Collection",
      progress: 0,
      statusLabel: "Draft",
      projects: [
        {
          id: "pine-listing",
          title: "Listing Onboarding",
          kind: "LISTING",
          status: "planning",
          progress: 0,
          phaseLabel: "Intake",
          workNodes: [
            { id: "pine-parties", title: "Clients / Parties", type: "group", status: "not-started" },
          ],
        },
      ],
    },
    {
      id: "harbor-lane",
      domain: "properties",
      label: "Harbor Lane",
      subtitle: "Coast Collection",
      progress: 0,
      statusLabel: "Draft",
      projects: [
        {
          id: "harbor-listing",
          title: "Listing Onboarding",
          kind: "LISTING",
          status: "planning",
          progress: 0,
          phaseLabel: "Intake",
          workNodes: [
            { id: "harbor-parties", title: "Clients / Parties", type: "group", status: "not-started" },
          ],
        },
      ],
    },
    {
      id: "jessica-iverson",
      domain: "people",
      label: "Jessica Iverson",
      subtitle: "Seller · Sea to Soul",
      progress: 67,
      statusLabel: "Active client",
      projects: [
        {
          id: "jessica-seller",
          title: "Seller Representation",
          kind: "CLIENT",
          status: "active",
          progress: 67,
          phaseLabel: "Listing",
          nextAction: "Execute listing agreement",
          workNodes: [
            { id: "jessica-agreement", title: "Sea to Soul agreement", type: "contract", status: "waiting" },
            { id: "jessica-follow-up", title: "Seller follow-up", type: "task", status: "in-progress" },
          ],
        },
      ],
    },
    {
      id: "johnsons",
      domain: "people",
      label: "The Johnsons",
      subtitle: "Buyer clients",
      progress: 42,
      statusLabel: "Active buyers",
      projects: [
        {
          id: "johnsons-buyer",
          title: "Buyer Representation",
          kind: "BUYER_REP",
          status: "active",
          progress: 42,
          phaseLabel: "Search",
          nextAction: "Review short list",
          workNodes: [
            { id: "johnsons-criteria", title: "Search criteria", type: "task", status: "complete" },
            { id: "johnsons-showings", title: "Showings", type: "appointment", status: "in-progress" },
          ],
        },
      ],
    },
    {
      id: "sea-to-soul-deal",
      domain: "deals",
      label: "Sea to Soul Listing",
      subtitle: "LISTING-01 v4",
      progress: 50,
      statusLabel: "Signature gate",
      projects: [
        {
          id: "sea-deal-execution",
          title: "Agreement Execution",
          kind: "DEAL",
          status: "active",
          progress: 50,
          phaseLabel: "Signature",
          nextAction: "Seller signature",
          workNodes: [
            { id: "deal-draft", title: "Final agreement", type: "document", status: "complete" },
            { id: "deal-signature", title: "Seller signature", type: "approval", status: "waiting" },
          ],
        },
      ],
    },
    {
      id: "culebraluxe-firm",
      domain: "firm",
      label: "CulebraLuxe",
      subtitle: "Firm projects",
      progress: 35,
      statusLabel: "Operating",
      projects: [
        {
          id: "mls-certification",
          title: "MLS Certification",
          kind: "FIRM",
          status: "active",
          progress: 60,
          phaseLabel: "Coursework",
          nextAction: "Attend MLS class",
          workNodes: [
            { id: "mls-registration", title: "Registration", type: "task", status: "complete" },
            { id: "mls-class", title: "MLS class", type: "appointment", status: "in-progress" },
          ],
        },
        {
          id: "website-redesign",
          title: "Website Redesign",
          kind: "INTERNAL",
          status: "active",
          progress: 15,
          phaseLabel: "Discovery",
          workNodes: [
            { id: "website-scope", title: "Scope", type: "task", status: "in-progress" },
          ],
        },
      ],
    },
    {
      id: "brand-marketing",
      domain: "marketing",
      label: "CulebraLuxe Marketing",
      subtitle: "Firm-wide campaigns",
      progress: 28,
      statusLabel: "Active",
      projects: [
        {
          id: "marketing-2027",
          title: "2027 Marketing Plan",
          kind: "MARKETING",
          status: "active",
          progress: 28,
          phaseLabel: "Plan",
          nextAction: "Finalize channel mix",
          workNodes: [
            { id: "marketing-strategy", title: "Strategy", type: "task", status: "complete" },
            { id: "marketing-channels", title: "Channel plan", type: "workflow", status: "in-progress" },
          ],
        },
      ],
    },
    {
      id: "brokerage-accounting",
      domain: "accounting",
      label: "Brokerage Accounting",
      subtitle: "Firm financial work",
      progress: 45,
      statusLabel: "Current month",
      projects: [
        {
          id: "september-close",
          title: "September Close",
          kind: "ACCOUNTING",
          status: "active",
          progress: 45,
          phaseLabel: "Reconcile",
          nextAction: "Review open receivables",
          workNodes: [
            { id: "acct-receivables", title: "Receivables", type: "accounting", status: "in-progress" },
            { id: "acct-expenses", title: "Expenses", type: "accounting", status: "in-progress" },
          ],
        },
      ],
    },
  ],
}
