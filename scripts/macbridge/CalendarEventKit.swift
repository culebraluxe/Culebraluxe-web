// ---------------------------------------------------------------------------
// MAC-SYNC-CAL-01 — Apple Calendar EventKit bridge (the Mac is the edge).
//
// Reads the Mac user's Apple/iCloud calendars via EventKit and writes a bounded
// normalized snapshot (JSON) that the CulebraLuxe CalendarEventSource seam
// consumes. Apple Calendar stays authoritative; CulebraLuxe only consumes.
//
//   Apple Calendar / iCloud
//     -> macOS EventKit (TCC calendar consent)
//     -> this bridge: bounded window, stable source ids, normalize
//     -> JSON snapshot file (default /tmp/culebraluxe-calendar.json)
//     -> lib/catchup/eventkit.ts -> CalendarEventSource -> Catch-Up calendar
//
// Minimal permission: requestFullAccessToEvents (calendar read). No CalDAV, no
// Apple ID/password, no Nylas, no fake REST API, no browser credentials.
//
// Idempotency: each event carries its stable EKEvent.eventIdentifier, so a
// re-run yields the same source id (no duplicates) and re-reading reflects
// edits; a deleted event simply no longer appears in the next snapshot.
//
// Run: swift CalendarEventKit.swift --out <path> --past-days 7 --future-days 60
// ---------------------------------------------------------------------------

import EventKit
import Foundation

// --- args ------------------------------------------------------------------
var outPath = "/tmp/culebraluxe-calendar.json"
var pastDays = 7
var futureDays = 60

var args = Array(CommandLine.arguments.dropFirst())
var i = 0
while i < args.count {
  switch args[i] {
  case "--out":
    i += 1
    if i < args.count { outPath = args[i] }
  case "--past-days":
    i += 1
    if i < args.count { pastDays = Int(args[i]) ?? 7 }
  case "--future-days":
    i += 1
    if i < args.count { futureDays = Int(args[i]) ?? 60 }
  default:
    break
  }
  i += 1
}

// --- authorization ----------------------------------------------------------
let store = EKEventStore()
let sem = DispatchSemaphore(value: 0)
var granted = false
var deniedError: String? = nil

store.requestFullAccessToEvents { ok, err in
  granted = ok
  if let e = err { deniedError = e.localizedDescription }
  sem.signal()
}
_ = sem.wait(timeout: .now() + 30)

print("status=\(granted ? "granted" : "denied")"
  + (deniedError.map { " err=\($0)" } ?? ""))

// If not granted, write an empty snapshot so the seam has a known state and we
// never fabricate events.
guard granted else {
  try? "[]".write(toFile: outPath, atomically: true, encoding: .utf8)
  exit(0)
}

// --- bounded window + read ---------------------------------------------------
let calendars = store.calendars(for: .event)
let start = Date().addingTimeInterval(-Double(pastDays) * 86_400)
let end = Date().addingTimeInterval(Double(futureDays) * 86_400)
let events = store.events(
  matching: store.predicateForEvents(
    withStart: start,
    end: end,
    calendars: calendars
  )
)

// --- normalize (no EventKit objects leak past this bridge) -------------------
let iso = ISO8601DateFormatter()
iso.formatOptions = [.withInternetDateTime]

struct BridgeEvent: Codable {
  let eventIdentifier: String
  let sourceAccount: String
  let calendarName: String
  let title: String
  let startAt: String
  let endAt: String
  let allDay: Bool
  let location: String?
  let notes: String?
}

let items = events.map { e -> BridgeEvent in
  BridgeEvent(
    eventIdentifier: e.eventIdentifier,
    sourceAccount: e.calendar?.source?.sourceIdentifier ?? "apple-calendar",
    calendarName: e.calendar?.title ?? "",
    title: e.title ?? "",
    startAt: iso.string(from: e.startDate),
    endAt: iso.string(from: e.endDate),
    allDay: e.isAllDay,
    location: e.location,
    notes: e.notes
  )
}

let encoder = JSONEncoder()
encoder.outputFormatting = [.prettyPrinted]
let data = (try? encoder.encode(items)) ?? Data("[]".utf8)
try? data.write(to: URL(fileURLWithPath: outPath))
print("events=\(items.count) out=\(outPath)")
