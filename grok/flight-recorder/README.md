# Flight Recorder — starting slice

Drop `src/` into a Next.js App Router app (Tailwind required).

## Wire-up

```tsx
// app/flight-recorder/[correlationId]/page.tsx
import { FlightRecorderPage } from '@/flight-recorder/FlightRecorderPage';

export default function Page() {
  return <FlightRecorderPage />;
}
```

Install:

```bash
npm i @tanstack/react-virtual
```

The page is a client component. It reads/writes:

```
?event=<id>&tab=timeline|causality|swimlane|raw&density=compact|expanded&q=&kinds=
```

Default selection is `evt_91c2d3e4f5b6a7c8` (Task Created), matching the mock.

## Files

| File | Role |
|---|---|
| `src/types.ts` | Domain model + color tokens |
| `src/fixture.ts` | All 18 events from the screenshot |
| `src/fixtures/deal-2025-000123.json` | Same data as raw JSON |
| `src/useFlightRecorderState.ts` | URL-backed selection / filters |
| `src/FlightRecorderPage.tsx` | Shell + virtualized timeline + details |
| `src/KindGlyph.tsx` | Kind icons |
| `src/format.ts` | Clock / offset / duration formatters |

## Still stubbed (next PR)

- Real ELK/dagre trace map with edges
- Causality / swimlane / raw tabs
- SSE live append
- Export / download / share clipboard
- Auto-refresh poll
