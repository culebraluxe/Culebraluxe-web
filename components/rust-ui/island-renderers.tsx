'use client'

import { moveStoryBucketAction } from '@/app/portal/tech/actions'
import {
  StoryKanbanBoard,
  type StoryKanbanCard,
  type StoryKanbanColumn,
} from '@/components/portal/tech/story-kanban-board'
import { FlightRecorderPage } from '@/components/portal/tech/flight-recorder-console/FlightRecorderPage'
import { FramerUiLab } from '@/components/portal/tech/framer-ui-lab'
import { TypeScriptUiLab } from '@/components/portal/tech/typescript-ui-lab'

import { adaptFlightRecorderTransaction } from '@/lib/flight-recorder-adapter'
import type { FlightRecorderTransaction } from '@/lib/flight-recorder-contract'

import type { IslandRenderer } from './island-host'

function FlightRecorderIsland({ transaction }: { transaction: FlightRecorderTransaction }) {
  const trace = adaptFlightRecorderTransaction(transaction)
  return <FlightRecorderPage trace={trace} defaultEventId={trace.events[0]?.id} />
}

// ---------------------------------------------------------------------------
// Every vendor widget a Yew screen can place with `<Island kind="...">`, by kind. A renderer draws from its props and
// reports events with `emit`; it reads no DOM outside its node and calls no API (the screen does).
// ---------------------------------------------------------------------------

type SorterProps = { cards: StoryKanbanCard[]; columns: StoryKanbanColumn[] }

/**
 * The story sorter's drag mechanics. KNOWN GAP (docs/agent/UI-SCREEN-ARCHITECTURE.md): the drop still calls its server
 * action from here, as it did before the port; everything else — refresh, selection, commands — is the screen's.
 * A successful drop stays local until the Cockpit's periodic re-read (re-initializing the board on every drop bounced
 * cards backwards); a failed one asks the screen to re-read at once.
 */
function SorterIsland({ props, emit }: { props: SorterProps; emit: (event: Record<string, unknown>) => void }) {
  return (
    <StoryKanbanBoard
      cards={props.cards}
      columns={props.columns}
      onMove={async (cardId, from, to) => {
        const result = await moveStoryBucketAction(cardId, from, to)
        if (!result.ok) emit({ type: 'refresh' })
        return result.ok ? { ...result, note: result.note ?? cardId + ' -> ' + to } : result
      }}
      onResync={() => emit({ type: 'refresh' })}
    />
  )
}

export const ISLAND_RENDERERS: Record<string, IslandRenderer> = {
  // TECH · UI Lab — the preserved galleries, unchanged.
  'ui-lab-gallery': () => <TypeScriptUiLab />,
  'ui-lab-motion': () => <FramerUiLab />,
  // TECH · Cockpit — the story sorter.
  'tech-sorter': (props, emit) => <SorterIsland props={props as SorterProps} emit={emit} />,
  // TECH · Flight Recorder — the console over one transaction snapshot; Yew owns the read and its refresh.
  'flight-recorder': (props) => <FlightRecorderIsland transaction={props as FlightRecorderTransaction} />,
}
