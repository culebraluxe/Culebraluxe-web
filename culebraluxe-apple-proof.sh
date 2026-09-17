#!/bin/zsh

osascript <<'APPLESCRIPT'

-- ============================================================
-- CulebraLuxe Apple Write Proof
--
-- Creates:
--   1. One Apple Reminder due today
--   2. One Apple Calendar event today
--
-- NO direct SQLite writes.
-- NO CulebraLuxe DB writes.
-- This is only a proof that we can push into Apple's ecosystem.
-- ============================================================

set nowDate to current date

-- Put the Reminder 10 minutes from now.
set reminderDue to nowDate + (10 * minutes)

-- Put the Calendar event 20 minutes from now for 30 minutes.
set eventStart to nowDate + (20 * minutes)
set eventEnd to eventStart + (30 * minutes)

set timestampText to do shell script "date '+%Y-%m-%d %H:%M:%S'"

set reminderTitle to "CulebraLuxe Proof — Call Jessica"
set eventTitle to "CulebraLuxe Proof — Listing Appointment"

set proofNotes to "CulebraLuxe Apple integration proof
Created: " & timestampText & "
Source: CulebraLuxe Apple Bridge prototype
Test ID: CL-APPLE-PROOF-001"

-- ============================================================
-- REMINDERS
-- ============================================================

tell application "Reminders"
	set reminderListNames to name of every list
end tell

if (count of reminderListNames) is 0 then
	display dialog "No Reminder lists were found." buttons {"Stop"} default button "Stop"
	error number -128
end if

set reminderChoice to choose from list reminderListNames with prompt ¬
	"Choose the iCloud Reminder list that should receive the CulebraLuxe test task:" ¬
	with title "CulebraLuxe — Reminder Target"

if reminderChoice is false then error number -128

set reminderListName to item 1 of reminderChoice

tell application "Reminders"
	set targetReminderList to first list whose name is reminderListName
	
	tell targetReminderList
		set newReminder to make new reminder at end with properties {¬
			name:reminderTitle, ¬
			body:proofNotes, ¬
			due date:reminderDue, ¬
			remind me date:reminderDue}
	end tell
	
	set reminderID to id of newReminder
end tell


-- ============================================================
-- CALENDAR
-- ============================================================

set writableCalendarNames to {}

tell application "Calendar"
	repeat with thisCalendar in calendars
		try
			if writable of thisCalendar is true then
				set thisName to name of thisCalendar
				if writableCalendarNames does not contain thisName then
					set end of writableCalendarNames to thisName
				end if
			end if
		end try
	end repeat
end tell

if (count of writableCalendarNames) is 0 then
	display dialog "No writable Calendars were found." buttons {"Stop"} default button "Stop"
	error number -128
end if

set calendarChoice to choose from list writableCalendarNames with prompt ¬
	"Choose the iCloud Calendar that should receive the CulebraLuxe test event:" ¬
	with title "CulebraLuxe — Calendar Target"

if calendarChoice is false then error number -128

set calendarName to item 1 of calendarChoice

tell application "Calendar"
	set targetCalendar to first calendar whose name is calendarName
	
	tell targetCalendar
		set newEvent to make new event at end of events with properties {¬
			summary:eventTitle, ¬
			start date:eventStart, ¬
			end date:eventEnd, ¬
			description:proofNotes, ¬
			location:"CulebraLuxe Apple Bridge Test"}
	end tell
	
	set eventID to id of newEvent
end tell


-- ============================================================
-- RESULT
-- ============================================================

set reminderTimeText to time string of reminderDue
set eventStartText to time string of eventStart
set eventEndText to time string of eventEnd

display dialog ¬
	"CulebraLuxe Apple proof COMPLETE." & return & return & ¬
	"REMINDER" & return & ¬
	"List: " & reminderListName & return & ¬
	"Task: " & reminderTitle & return & ¬
	"Due: " & reminderTimeText & return & ¬
	"ID: " & reminderID & return & return & ¬
	"CALENDAR EVENT" & return & ¬
	"Calendar: " & calendarName & return & ¬
	"Event: " & eventTitle & return & ¬
	"Time: " & eventStartText & " – " & eventEndText & return & ¬
	"ID: " & eventID & return & return & ¬
	"Now check the Mac, iPhone and iPad." ¬
	buttons {"Sweet"} default button "Sweet" ¬
	with title "CulebraLuxe Apple Bridge"

APPLESCRIPT