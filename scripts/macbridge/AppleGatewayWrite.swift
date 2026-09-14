// ---------------------------------------------------------------------------
// CulebraLuxe Apple gateway — explicit outbound EventKit writes.
//
// Input is a private temporary JSON file created by the Mac gateway worker.
// Nothing sensitive is passed on the process command line or printed.
//
// Supported commands:
//   calendar_create  -> EKEvent
//   reminder_upsert  -> EKReminder (stable by CulebraLuxe WBS marker)
// ---------------------------------------------------------------------------

import EventKit
import Foundation

struct GatewayCommand: Codable {
  let kind: String
  let commandId: String
  let wbsId: String?
  let title: String
  let startAt: String?
  let endAt: String?
  let allDay: Bool?
  let location: String?
  let notes: String?
  let dueAt: String?
  let completed: Bool?
}

func fail(_ message: String) -> Never {
  FileHandle.standardError.write(Data((message + "\n").utf8))
  exit(1)
}

let args = Array(CommandLine.arguments.dropFirst())
guard args.count == 2, args[0] == "--input" else {
  fail("usage: AppleGatewayWrite.swift --input <private-json-file>")
}

let inputURL = URL(fileURLWithPath: args[1])
guard let data = try? Data(contentsOf: inputURL),
      let command = try? JSONDecoder().decode(GatewayCommand.self, from: data) else {
  fail("apple-gateway-write: invalid input")
}

let iso = ISO8601DateFormatter()
iso.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
let isoFallback = ISO8601DateFormatter()
isoFallback.formatOptions = [.withInternetDateTime]
func parseDate(_ value: String?) -> Date? {
  guard let value else { return nil }
  return iso.date(from: value) ?? isoFallback.date(from: value)
}

let store = EKEventStore()

func requestEvents() -> Bool {
  let sem = DispatchSemaphore(value: 0)
  var allowed = false
  store.requestFullAccessToEvents { granted, _ in
    allowed = granted
    sem.signal()
  }
  _ = sem.wait(timeout: .now() + 30)
  return allowed
}

func requestReminders() -> Bool {
  let sem = DispatchSemaphore(value: 0)
  var allowed = false
  store.requestFullAccessToReminders { granted, _ in
    allowed = granted
    sem.signal()
  }
  _ = sem.wait(timeout: .now() + 30)
  return allowed
}

func writableCalendar(for type: EKEntityType) -> EKCalendar? {
  if type == .event, let calendar = store.defaultCalendarForNewEvents, calendar.allowsContentModifications {
    return calendar
  }
  if type == .reminder, let calendar = store.defaultCalendarForNewReminders(), calendar.allowsContentModifications {
    return calendar
  }
  return store.calendars(for: type).first(where: { $0.allowsContentModifications })
}

func mergedNotes(_ userNotes: String?, marker: String) -> String {
  let clean = (userNotes ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
  return clean.isEmpty ? marker : clean + "\n\n" + marker
}

switch command.kind {
case "calendar_create":
  guard requestEvents() else { fail("apple-gateway-write: calendar access denied") }
  guard let start = parseDate(command.startAt), let end = parseDate(command.endAt), end > start else {
    fail("apple-gateway-write: invalid calendar start/end")
  }
  guard let calendar = writableCalendar(for: .event) else {
    fail("apple-gateway-write: no writable event calendar")
  }

  let marker = "CulebraLuxe Command: \(command.commandId)"
  let searchStart = start.addingTimeInterval(-86_400)
  let searchEnd = end.addingTimeInterval(86_400)
  let predicate = store.predicateForEvents(withStart: searchStart, end: searchEnd, calendars: nil)
  if let existing = store.events(matching: predicate).first(where: { ($0.notes ?? "").contains(marker) }) {
    print("result=existing kind=calendar_create external_id=\(existing.eventIdentifier ?? "")")
    exit(0)
  }

  let event = EKEvent(eventStore: store)
  event.calendar = calendar
  event.title = command.title
  event.startDate = start
  event.endDate = end
  event.isAllDay = command.allDay ?? false
  event.location = command.location
  event.notes = mergedNotes(command.notes, marker: marker)
  do {
    try store.save(event, span: .thisEvent, commit: true)
    print("result=created kind=calendar_create external_id=\(event.eventIdentifier ?? "")")
  } catch {
    fail("apple-gateway-write: calendar save failed: \(error.localizedDescription)")
  }

case "reminder_upsert":
  guard requestReminders() else { fail("apple-gateway-write: reminders access denied") }
  guard let wbsId = command.wbsId, !wbsId.isEmpty else {
    fail("apple-gateway-write: reminder wbsId required")
  }
  guard let calendar = writableCalendar(for: .reminder) else {
    fail("apple-gateway-write: no writable reminder list")
  }

  let marker = "CulebraLuxe WBS: \(wbsId)"
  let sem = DispatchSemaphore(value: 0)
  var reminders: [EKReminder] = []
  let predicate = store.predicateForReminders(in: nil)
  _ = store.fetchReminders(matching: predicate) { found in
    reminders = found ?? []
    sem.signal()
  }
  _ = sem.wait(timeout: .now() + 30)

  let reminder = reminders.first(where: { ($0.notes ?? "").contains(marker) }) ?? EKReminder(eventStore: store)
  if reminder.calendar == nil { reminder.calendar = calendar }
  reminder.title = command.title
  reminder.notes = mergedNotes(command.notes, marker: marker)
  reminder.isCompleted = command.completed ?? false

  if let due = parseDate(command.dueAt) {
    var components = Calendar(identifier: .gregorian).dateComponents(
      [.year, .month, .day, .hour, .minute, .second],
      from: due
    )
    components.calendar = Calendar(identifier: .gregorian)
    components.timeZone = TimeZone(identifier: "America/Puerto_Rico")
    reminder.dueDateComponents = components
  } else {
    reminder.dueDateComponents = nil
  }

  do {
    let existed = reminder.eventIdentifier != nil
    try store.save(reminder, commit: true)
    print("result=\(existed ? "updated" : "created") kind=reminder_upsert external_id=\(reminder.eventIdentifier ?? "")")
  } catch {
    fail("apple-gateway-write: reminder save failed: \(error.localizedDescription)")
  }

default:
  fail("apple-gateway-write: unsupported command kind")
}
