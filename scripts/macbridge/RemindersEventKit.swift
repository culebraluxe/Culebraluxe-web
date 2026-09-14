// ---------------------------------------------------------------------------
// MAC-SYNC-REM-01 — Apple Reminders EventKit bridge (the Mac is the edge).
//
// Apple presents scheduled reminders inside Calendar, but EventKit models them
// separately from EKEvent. This bridge reads EKReminder only and writes a
// normalized snapshot for the CulebraLuxe Work side.
//
//   Apple Reminders / scheduled reminders shown in Calendar
//     -> macOS EventKit (separate Reminders TCC consent)
//     -> this bridge
//     -> /tmp/culebraluxe-reminders.json
//     -> scripts/apple-reminders-intake.ts
//     -> l_reminder
//
// CulebraLuxe does not create WBS rows here. Apple remains authoritative for
// these external reminders; canonical WBS work remains a separate domain.
// ---------------------------------------------------------------------------

import EventKit
import Foundation

var outPath = "/tmp/culebraluxe-reminders.json"
var args = Array(CommandLine.arguments.dropFirst())
var i = 0
while i < args.count {
  if args[i] == "--out" {
    i += 1
    if i < args.count { outPath = args[i] }
  }
  i += 1
}

let store = EKEventStore()
let permission = DispatchSemaphore(value: 0)
var granted = false
var deniedError: String? = nil

store.requestFullAccessToReminders { ok, err in
  granted = ok
  if let e = err { deniedError = e.localizedDescription }
  permission.signal()
}
_ = permission.wait(timeout: .now() + 30)

print("reminders-status=\(granted ? "granted" : "denied")"
  + (deniedError.map { " err=\($0)" } ?? ""))

guard granted else {
  try? "[]".write(toFile: outPath, atomically: true, encoding: .utf8)
  exit(0)
}

let calendars = store.calendars(for: .reminder)
let predicate = store.predicateForReminders(in: calendars)
let fetched = DispatchSemaphore(value: 0)
var reminders: [EKReminder] = []

_ = store.fetchReminders(matching: predicate) { items in
  reminders = items ?? []
  fetched.signal()
}
_ = fetched.wait(timeout: .now() + 30)

let iso = ISO8601DateFormatter()
iso.formatOptions = [.withInternetDateTime]

func isoDate(_ components: DateComponents?) -> String? {
  guard var c = components else { return nil }
  var calendar = c.calendar ?? Calendar.current
  if c.timeZone == nil { c.timeZone = calendar.timeZone }
  return calendar.date(from: c).map { iso.string(from: $0) }
}

struct BridgeReminder: Codable {
  let reminderIdentifier: String
  let externalIdentifier: String?
  let sourceAccount: String
  let listName: String
  let title: String
  let notes: String?
  let startAt: String?
  let dueAt: String?
  let completed: Bool
  let completedAt: String?
  let priority: Int
}

let items = reminders.map { reminder -> BridgeReminder in
  BridgeReminder(
    reminderIdentifier: reminder.calendarItemIdentifier,
    externalIdentifier: reminder.calendarItemExternalIdentifier,
    sourceAccount: reminder.calendar?.source?.sourceIdentifier ?? "apple-reminders",
    listName: reminder.calendar?.title ?? "",
    title: reminder.title ?? "",
    notes: reminder.notes,
    startAt: isoDate(reminder.startDateComponents),
    dueAt: isoDate(reminder.dueDateComponents),
    completed: reminder.isCompleted,
    completedAt: reminder.completionDate.map { iso.string(from: $0) },
    priority: reminder.priority
  )
}

let encoder = JSONEncoder()
encoder.outputFormatting = [.prettyPrinted]
let data = (try? encoder.encode(items)) ?? Data("[]".utf8)
try? data.write(to: URL(fileURLWithPath: outPath))
print("reminders=\(items.count) out=\(outPath)")
