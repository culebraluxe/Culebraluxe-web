import type { Client, Deal } from "./types"

export const mockClients: Client[] = [
  {
    id: "client-ana-rivera",
    displayName: "Ana Rivera",
    role: "buyer",
    status: "warm",
    location: "San Juan, Puerto Rico",
    email: "ana@example.com",
    phone: "+1 787 555 0101",

    budgetMin: 2_000_000,
    budgetMax: 3_500_000,

    preferredAreas: [
      "Flamenco",
      "Zoni",
      "Carlos Rosario",
    ],

    propertyTypes: [
      "Villa",
      "Ocean-view residence",
    ],

    priorities: [
      "Privacy",
      "Turnkey",
      "Rental potential",
    ],

    timeline: "3–6 months",
    assignedAgent: "Lisa Penfield",

    lastContact: {
      channel: "call",
      occurredAt: "Aug 15 · 11:42 AM",
      summary: "Discussed Villa Mar Azul and timing for another visit.",
    },

    nextAction: {
      title: "Property tour",
      occurredAt: "Aug 18 · 10:30 AM",
      detail: "Villa Mar Azul",
    },

    notes:
      "Looking for a private island retreat with strong indoor-outdoor living. Prefers modern finishes and turnkey condition. Family visits Culebra regularly.",

    propertyInterests: [
      {
        id: "interest-1",
        propertyId: "property-villa-mar-azul",
        propertyName: "Villa Mar Azul",
        location: "Flamenco",
        price: 2_450_000,
        bedrooms: 3,
        descriptor: "Ocean view",
        status: "tour_completed",
      },
      {
        id: "interest-2",
        propertyId: "property-casa-brisa",
        propertyName: "Casa Brisa",
        location: "Zoni",
        price: 2_150_000,
        bedrooms: 4,
        descriptor: "Hillside residence",
        status: "shortlisted",
      },
      {
        id: "interest-3",
        propertyId: "property-sunset-point",
        propertyName: "Sunset Point",
        location: "Punta Soldado",
        price: 2_795_000,
        bedrooms: 5,
        descriptor: "Sunset views",
        status: "interested",
      },
    ],

    interactions: [
      {
        id: "interaction-1",
        channel: "call",
        eventType: "call",
        sourceMetadata: {},
        direction: "outbound",
        occurredAt: "Aug 15 · 11:42 AM",
        title: "Phone call",
        summary:
          "Discussed Villa Mar Azul and timing for another visit.",
      },
      {
        id: "interaction-2",
        channel: "email",
        eventType: "email",
        sourceMetadata: {},
        direction: "outbound",
        occurredAt: "Aug 14 · 8:16 AM",
        title: "Email",
        summary:
          "Sent updated property information and availability.",
      },
      {
        id: "interaction-3",
        channel: "showing",
        eventType: "showing",
        sourceMetadata: {},
        occurredAt: "Aug 11 · 4:30 PM",
        title: "Property showing",
        summary:
          "Viewed Sunset Point and Casa Brisa.",
      },
      {
        id: "interaction-4",
        channel: "note",
        eventType: "note",
        sourceMetadata: {},
        occurredAt: "Aug 10 · 9:05 AM",
        title: "Relationship note",
        summary:
          "Strong preference for privacy, modern finishes and western exposure.",
      },
    ],
  },

  {
    id: "client-felipe-ortega",
    displayName: "Felipe Ortega",
    role: "buyer",
    status: "active",
    location: "Miami, Florida",
    email: "felipe@example.com",
    phone: "+1 305 555 0188",

    budgetMin: 1_500_000,
    budgetMax: 2_500_000,

    preferredAreas: ["Dewey", "Flamenco"],
    propertyTypes: ["Villa"],
    priorities: ["Water access", "Rental income"],
    timeline: "1–3 months",

    assignedAgent: "Lisa Penfield",

    lastContact: {
      channel: "email",
      occurredAt: "Aug 14 · 3:20 PM",
      summary: "Sent Casa Solana availability.",
    },

    nextAction: {
      title: "Follow-up call",
      occurredAt: "Aug 16 · 9:30 AM",
      detail: "Discuss Casa Solana",
    },

    notes:
      "Active buyer. Familiar with Puerto Rico and focused on properties capable of occasional rental income.",

    propertyInterests: [
      {
        id: "interest-4",
        propertyId: "property-casa-solana",
        propertyName: "Casa Solana",
        location: "Dewey",
        price: 1_875_000,
        bedrooms: 4,
        descriptor: "Harbor views",
        status: "shortlisted",
      },
    ],

    interactions: [
      {
        id: "interaction-5",
        channel: "email",
        eventType: "email",
        sourceMetadata: {},
        direction: "outbound",
        occurredAt: "Aug 14 · 3:20 PM",
        title: "Email",
        summary: "Sent Casa Solana availability.",
      },
      {
        id: "interaction-6",
        channel: "call",
        eventType: "call",
        sourceMetadata: {},
        direction: "inbound",
        occurredAt: "Aug 12 · 1:10 PM",
        title: "Phone call",
        summary: "Asked about rental history and insurance.",
      },
    ],
  },

  {
    id: "client-james-lee",
    displayName: "James Lee",
    role: "seller",
    status: "active",
    location: "Boston, Massachusetts",
    email: "james@example.com",

    assignedAgent: "Lisa Penfield",

    lastContact: {
      channel: "meeting",
      occurredAt: "Aug 13 · 2:00 PM",
      summary: "Listing strategy review.",
    },

    nextAction: {
      title: "Send valuation",
      occurredAt: "Aug 16",
      detail: "Brisas del Mar",
    },

    notes:
      "Preparing Brisas del Mar for market. Photography and pricing strategy still under review.",

    propertyInterests: [],

    interactions: [
      {
        id: "interaction-7",
        channel: "meeting",
        eventType: "meeting",
        sourceMetadata: {},
        occurredAt: "Aug 13 · 2:00 PM",
        title: "Listing strategy",
        summary: "Reviewed timing, photography and initial pricing.",
      },
    ],
  },

  {
    id: "client-wilson",
    displayName: "Laura & Mark Wilson",
    role: "buyer",
    status: "warm",
    location: "New York, New York",

    budgetMin: 2_500_000,
    budgetMax: 4_000_000,

    preferredAreas: ["Flamenco", "Punta Soldado"],
    priorities: ["Sunset views", "Privacy"],
    timeline: "6–12 months",

    assignedAgent: "Lisa Penfield",

    lastContact: {
      channel: "imessage",
      occurredAt: "Aug 15 · 9:06 AM",
      summary: "Asked about timing for a return visit.",
    },

    nextAction: {
      title: "Send property shortlist",
      occurredAt: "Aug 17",
    },

    notes:
      "Second-home buyers. Highly motivated by sunset exposure and architecture.",

    propertyInterests: [],

    interactions: [
      {
        id: "interaction-8",
        channel: "imessage",
        eventType: "imessage",
        sourceMetadata: {},
        direction: "inbound",
        occurredAt: "Aug 15 · 9:06 AM",
        title: "iMessage",
        summary: "Asked about timing for a return visit.",
      },
    ],
  },
]

export const mockDeals: Deal[] = [
  {
    id: "deal-villa-mar-azul",
    propertyId: "property-villa-mar-azul",
    propertyName: "Villa Mar Azul",
    propertyLocation: "Flamenco",
    clientId: "client-ana-rivera",
    clientName: "Ana Rivera",
    stage: "showing",
    listPrice: 2_450_000,
    nextMilestone: "Second property tour",
    nextMilestoneAt: "Aug 18 · 10:30 AM",
    lastActivity: "Phone call",
    lastActivityAt: "Aug 15 · 11:42 AM",
    owner: "Lisa Penfield",
    propertyDescriptor: "Ocean-view villa",
  },

  {
    id: "deal-casa-solana",
    propertyId: "property-casa-solana",
    propertyName: "Casa Solana",
    propertyLocation: "Dewey",
    clientId: "client-felipe-ortega",
    clientName: "Felipe Ortega",
    stage: "qualified",
    listPrice: 1_875_000,
    nextMilestone: "Buyer follow-up",
    nextMilestoneAt: "Aug 16 · 9:30 AM",
    lastActivity: "Email sent",
    lastActivityAt: "Aug 14 · 3:20 PM",
    owner: "Lisa Penfield",
    propertyDescriptor: "Harbor-view residence",
  },

  {
    id: "deal-sunset-point",
    propertyId: "property-sunset-point",
    propertyName: "Sunset Point",
    propertyLocation: "Punta Soldado",
    clientId: "client-wilson",
    clientName: "Laura & Mark Wilson",
    stage: "offer",
    listPrice: 3_250_000,
    offerPrice: 3_100_000,
    nextMilestone: "Seller response",
    nextMilestoneAt: "Aug 17 · EOD",
    lastActivity: "Offer submitted",
    lastActivityAt: "Aug 15 · 8:45 AM",
    owner: "Lisa Penfield",
    propertyDescriptor: "Sunset estate",
  },

  {
    id: "deal-brisas-del-mar",
    propertyId: "property-brisas-del-mar",
    propertyName: "Brisas del Mar",
    propertyLocation: "Dewey",
    clientId: "client-james-lee",
    clientName: "James Lee",
    stage: "new_lead",
    listPrice: 1_195_000,
    nextMilestone: "Complete valuation",
    nextMilestoneAt: "Aug 16",
    lastActivity: "Listing strategy meeting",
    lastActivityAt: "Aug 13 · 2:00 PM",
    owner: "Lisa Penfield",
    propertyDescriptor: "Seller opportunity",
  },

  {
    id: "deal-casa-brisa",
    propertyId: "property-casa-brisa",
    propertyName: "Casa Brisa",
    propertyLocation: "Zoni",
    clientId: "client-ana-rivera",
    clientName: "Ana Rivera",
    stage: "under_contract",
    listPrice: 2_150_000,
    offerPrice: 2_075_000,
    nextMilestone: "Inspection",
    nextMilestoneAt: "Aug 20 · 9:00 AM",
    lastActivity: "Contract executed",
    lastActivityAt: "Aug 14 · 5:10 PM",
    owner: "Lisa Penfield",
    closingDate: "Sep 12, 2026",
    propertyDescriptor: "Hillside residence",
  },

  {
    id: "deal-casa-horizonte",
    propertyId: "property-casa-horizonte",
    propertyName: "Casa Horizonte",
    propertyLocation: "Punta Melones",
    clientId: "client-wilson",
    clientName: "Laura & Mark Wilson",
    stage: "closed",
    listPrice: 1_650_000,
    offerPrice: 1_600_000,
    nextMilestone: "Closed",
    nextMilestoneAt: "Aug 8",
    lastActivity: "Closing completed",
    lastActivityAt: "Aug 8 · 3:15 PM",
    owner: "Lisa Penfield",
    closingDate: "Aug 8, 2026",
    propertyDescriptor: "Architectural residence",
  },
]
