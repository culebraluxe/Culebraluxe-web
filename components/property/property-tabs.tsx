'use client'

import {useState} from 'react'

type PropertyTabsProps = {
  property: {
    shortDescription?: string | null
    editorialDescription?: unknown
    amenities?: string[] | null
    latitude?: number | null
    longitude?: number | null
    gallery?: unknown[] | null
  }
}

const tabs = ['Overview', 'Details', 'Features', 'Location', 'Gallery'] as const

export function PropertyTabs({property}: PropertyTabsProps) {
  const [activeTab, setActiveTab] = useState<(typeof tabs)[number]>('Overview')

  return (
    <section
      style={{
        width: '100%',
        padding: '56px 0 96px',
      }}
    >
      <nav
        style={{
          display: 'flex',
          gap: '40px',
          borderBottom: '1px solid rgba(0,0,0,0.16)',
          marginBottom: '40px',
        }}
      >
        {tabs.map((tab) => (
          <button
            key={tab}
            type="button"
            onClick={() => setActiveTab(tab)}
            style={{
              appearance: 'none',
              border: 0,
              background: 'transparent',
              padding: '0 0 14px',
              margin: 0,
              cursor: 'pointer',
              fontSize: '12px',
              letterSpacing: '0.12em',
              textTransform: 'uppercase',
              borderBottom:
                activeTab === tab
                  ? '1px solid currentColor'
                  : '1px solid transparent',
            }}
          >
            {tab}
          </button>
        ))}
      </nav>

      {activeTab === 'Overview' && (
        <div>
          <p
            style={{
              fontFamily: 'var(--font-cormorant)',
              fontSize: '30px',
              lineHeight: 1.3,
              maxWidth: '760px',
              margin: 0,
            }}
          >
            {property.shortDescription ?? 'Property overview coming soon.'}
          </p>
        </div>
      )}

      {activeTab === 'Details' && (
        <p>Property details will appear here.</p>
      )}

      {activeTab === 'Features' && (
        <p>Property features will appear here.</p>
      )}

      {activeTab === 'Location' && (
        <p>Property location will appear here.</p>
      )}

      {activeTab === 'Gallery' && (
        <p>Property gallery will appear here.</p>
      )}
    </section>
  )
}