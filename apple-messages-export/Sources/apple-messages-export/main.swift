import Foundation
import SQLite3

// ===========================================================================
// CulebraLuxe — Apple Messages full-fidelity LOCAL exporter.
//
// Opens ~/Library/Messages/chat.db with SQLITE_OPEN_READONLY and streams a
// durable, versioned export package (manifest.json + JSONL) without ever
// writing to Apple's database, extracting attachment binaries, or logging
// message bodies to the console.
//
// Usage:
//   swift run apple-messages-export [--out <dir>] [--maxMessages <n>] [--maxHandles <n>]
// ===========================================================================

let home = FileManager.default.homeDirectoryForCurrentUser
let defaultDB = home
    .appendingPathComponent("Library")
    .appendingPathComponent("Messages")
    .appendingPathComponent("chat.db")

let sourceSystem = "apple_messages"
let exportVersion = 1

// Apple Messages timestamps are stored as NANOSECONDS since the Apple
// reference date (2001-01-01T00:00:00Z). Observed live example:
// 809312575831244416 ns -> 809312575.831 s since 2001 -> ~mid-2026.
// Convert to Unix epoch: raw / 1_000_000_000 + 978_307_200.
let APPLE_EPOCH_UNIX_OFFSET: Double = 978307200

struct Args {
    var outDir: URL?
    var maxMessages: Int?
    var maxHandles: Int?
}

func parseArgs() -> Args {
    let a = Array(CommandLine.arguments.dropFirst())
    var outDir: URL?
    var maxMessages: Int?
    var maxHandles: Int?
    var i = 0
    while i < a.count {
        switch a[i] {
        case "--out":
            if i + 1 < a.count { outDir = URL(fileURLWithPath: a[i + 1]); i += 1 }
        case "--maxMessages":
            if i + 1 < a.count { maxMessages = Int(a[i + 1]); i += 1 }
        case "--maxHandles":
            if i + 1 < a.count { maxHandles = Int(a[i + 1]); i += 1 }
        default:
            break
        }
        i += 1
    }
    return Args(outDir: outDir, maxMessages: maxMessages, maxHandles: maxHandles)
}

func fail(_ message: String) -> Never {
    FileHandle.standardError.write(Data("ERROR: \(message)\n".utf8))
    exit(EXIT_FAILURE)
}

func openReadOnly(_ url: URL) -> OpaquePointer {
    var db: OpaquePointer?
    let rc = sqlite3_open_v2(url.path, &db, SQLITE_OPEN_READONLY, nil)
    guard rc == SQLITE_OK, let db else {
        let msg = db.map { String(cString: sqlite3_errmsg($0)) } ?? "unknown"
        sqlite3_close(db)
        fail("cannot open Messages DB read-only: \(msg)")
    }
    return db
}

func availableColumns(_ db: OpaquePointer, _ table: String) -> Set<String> {
    var stmt: OpaquePointer?
    guard sqlite3_prepare_v2(db, "PRAGMA table_info(\(table));", -1, &stmt, nil) == SQLITE_OK else {
        return []
    }
    defer { sqlite3_finalize(stmt) }
    var cols = Set<String>()
    while sqlite3_step(stmt) == SQLITE_ROW {
        if let c = sqlite3_column_text(stmt, 1) {
            cols.insert(String(cString: c))
        }
    }
    return cols
}

func quoteIdent(_ s: String) -> String { "\"\(s.replacingOccurrences(of: "\"", with: "\"\""))\"" }

// Apple nanoseconds-since-2001 -> ISO-8601 string (or nil).
func appleTimestampToISO(_ raw: Double) -> String? {
    let unix = raw / 1_000_000_000 + APPLE_EPOCH_UNIX_OFFSET
    let date = Date(timeIntervalSince1970: unix)
    guard date.timeIntervalSince1970 > 0 else { return nil }
    let f = ISO8601DateFormatter()
    f.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
    return f.string(from: date)
}

// SQLite stores Apple timestamps as INTEGER nanoseconds; read as Double.
func timestampDouble(_ row: [String: Any], _ key: String) -> Double? {
    if let d = row[key] as? Double { return d }
    if let i = row[key] as? Int64 { return Double(i) }
    if let i = row[key] as? Int { return Double(i) }
    return nil
}

func stream(_ db: OpaquePointer, _ sql: String, _ perRow: ([String: Any]) -> Void) {
    var stmt: OpaquePointer?
    guard sqlite3_prepare_v2(db, sql, -1, &stmt, nil) == SQLITE_OK else {
        FileHandle.standardError.write(Data("prepare failed: \(String(cString: sqlite3_errmsg(db)))\n".utf8))
        sqlite3_finalize(stmt)
        return
    }
    defer { sqlite3_finalize(stmt) }
    while sqlite3_step(stmt) == SQLITE_ROW {
        perRow(readRow(stmt))
    }
}

func readRow(_ stmt: OpaquePointer?) -> [String: Any] {
    let n = sqlite3_column_count(stmt)
    var dict: [String: Any] = [:]
    for i in 0..<n {
        guard let name = sqlite3_column_name(stmt, i) else { continue }
        let key = String(cString: name)
        switch sqlite3_column_type(stmt, i) {
        case SQLITE_INTEGER:
            dict[key] = sqlite3_column_int64(stmt, i)
        case SQLITE_FLOAT:
            dict[key] = sqlite3_column_double(stmt, i)
        case SQLITE_TEXT:
            dict[key] = String(cString: sqlite3_column_text(stmt, i))
        default:
            dict[key] = NSNull()
        }
    }
    return dict
}

func jsonLine(_ obj: [String: Any]) -> String {
    guard let data = try? JSONSerialization.data(withJSONObject: obj, options: [.sortedKeys]),
          let line = String(data: data, encoding: .utf8)
    else {
        return "{}"
    }
    return line
}

func appendLine(_ handle: FileHandle, _ line: String) {
    handle.write(Data((line + "\n").utf8))
}

func openWrite(_ url: URL) -> FileHandle {
    FileManager.default.createFile(atPath: url.path, contents: nil)
    return FileHandle(forWritingAtPath: url.path)!
}

func selectExisting(_ db: OpaquePointer, _ table: String, _ preferred: [String]) -> String {
    let avail = availableColumns(db, table)
    let chosen = preferred.filter { avail.contains($0) }
    let cols = chosen.map { quoteIdent($0) }.joined(separator: ", ")
    return "SELECT \(cols) FROM \(table)"
}

func exportHandles(_ db: OpaquePointer, _ url: URL, _ maxHandles: Int?) -> Int {
    let f = openWrite(url)
    var count = 0
    let preferred = ["ROWID", "id", "country", "service", "uncanonicalized_id", "person_centric_id"]
    let limit = maxHandles.map { " LIMIT \($0)" } ?? ""
    let sql = selectExisting(db, "handle", preferred) + " ORDER BY ROWID" + limit
    stream(db, sql) { row in
        let obj: [String: Any] = [
            "rowid": row["ROWID"] ?? NSNull(),
            "id": row["id"] ?? NSNull(),
            "country": row["country"] ?? NSNull(),
            "service": row["service"] ?? NSNull(),
            "uncanonicalizedId": row["uncanonicalized_id"] ?? NSNull(),
            "personCentricId": row["person_centric_id"] ?? NSNull(),
        ]
        appendLine(f, jsonLine(obj))
        count += 1
    }
    f.closeFile()
    return count
}

func exportChats(_ db: OpaquePointer, _ url: URL) -> Int {
    let f = openWrite(url)
    var count = 0
    let preferred = ["ROWID", "guid", "style", "state", "chat_identifier", "service_name", "display_name", "group_id", "is_archived", "last_read_message_timestamp", "account_login"]
    let sql = selectExisting(db, "chat", preferred) + " ORDER BY ROWID"
    stream(db, sql) { row in
        let lrms = timestampDouble(row, "last_read_message_timestamp")
        let obj: [String: Any] = [
            "rowid": row["ROWID"] ?? NSNull(),
            "guid": row["guid"] ?? NSNull(),
            "style": row["style"] ?? NSNull(),
            "state": row["state"] ?? NSNull(),
            "chatIdentifier": row["chat_identifier"] ?? NSNull(),
            "serviceName": row["service_name"] ?? NSNull(),
            "displayName": row["display_name"] ?? NSNull(),
            "groupId": row["group_id"] ?? NSNull(),
            "isArchived": row["is_archived"] ?? NSNull(),
            "lastReadMessageTimestamp": lrms.map(appleTimestampToISO) ?? NSNull(),
            "accountLogin": row["account_login"] ?? NSNull(),
        ]
        appendLine(f, jsonLine(obj))
        count += 1
    }
    f.closeFile()
    return count
}

func exportChatParticipants(_ db: OpaquePointer, _ url: URL) -> Int {
    let f = openWrite(url)
    var count = 0
    let sql = "SELECT chj.chat_id, ch.guid AS chat_guid, chj.handle_id, h.id AS handle_value, h.service AS handle_service FROM chat_handle_join chj JOIN chat ch ON ch.ROWID = chj.chat_id JOIN handle h ON h.ROWID = chj.handle_id ORDER BY chj.chat_id, chj.handle_id"
    stream(db, sql) { row in
        let obj: [String: Any] = [
            "chatId": row["chat_id"] ?? NSNull(),
            "chatGuid": row["chat_guid"] ?? NSNull(),
            "handleId": row["handle_id"] ?? NSNull(),
            "handleValue": row["handle_value"] ?? NSNull(),
            "handleService": row["handle_service"] ?? NSNull(),
        ]
        appendLine(f, jsonLine(obj))
        count += 1
    }
    f.closeFile()
    return count
}


struct MessageStats {
    var count = 0
    var withText = 0
    var minISO: String?
    var maxISO: String?
}

func exportMessages(_ db: OpaquePointer, _ url: URL, _ maxMessages: Int?) -> MessageStats {
    let f = openWrite(url)
    var stats = MessageStats()
    let preferred = [
        "guid", "handle_id", "service", "account", "account_guid", "date", "date_read",
        "date_delivered", "is_from_me", "is_read", "is_sent", "is_delivered", "is_finished",
        "is_empty", "is_system_message", "is_service_message", "cache_has_attachments",
        "is_audio_message", "item_type", "associated_message_guid", "associated_message_type",
        "reply_to_guid", "thread_originator_guid", "date_retracted", "date_edited", "is_spam",
        "schedule_type", "sent_or_received_off_grid", "text",
    ]
    let avail = availableColumns(db, "message")
    let chosen = preferred.filter { avail.contains($0) }
    let cols = chosen.map { "m." + quoteIdent($0) }.joined(separator: ", ")
    let limit = maxMessages.map { " LIMIT \($0)" } ?? ""
    let sql = "SELECT m.ROWID AS rowid, \(cols), cmj.chat_id AS chat_id, c.guid AS chat_guid, h.id AS handle_value FROM message m LEFT JOIN chat_message_join cmj ON cmj.message_id = m.ROWID LEFT JOIN chat c ON c.ROWID = cmj.chat_id LEFT JOIN handle h ON h.ROWID = m.handle_id ORDER BY m.ROWID" + limit
    stream(db, sql) { row in
        let date = timestampDouble(row, "date")
        let iso = date.flatMap(appleTimestampToISO)
        if let iso {
            if stats.minISO == nil || iso < stats.minISO! { stats.minISO = iso }
            if stats.maxISO == nil || iso > stats.maxISO! { stats.maxISO = iso }
        }
        var text: Any = NSNull()
        if let t = row["text"] as? String, !t.isEmpty {
            text = t
            stats.withText += 1
        }
        let obj: [String: Any] = [
            "rowid": row["rowid"] ?? NSNull(),
            "guid": row["guid"] ?? NSNull(),
            "chatGuid": row["chat_guid"] ?? NSNull(),
            "handleId": row["handle_id"] ?? NSNull(),
            "handleValue": row["handle_value"] ?? NSNull(),
            "service": row["service"] ?? NSNull(),
            "account": row["account"] ?? NSNull(),
            "date": date ?? NSNull(),
            "dateISO": iso ?? NSNull(),
            "dateReadISO": timestampDouble(row, "date_read").flatMap(appleTimestampToISO) ?? NSNull(),
            "dateDeliveredISO": timestampDouble(row, "date_delivered").flatMap(appleTimestampToISO) ?? NSNull(),
            "isFromMe": row["is_from_me"] ?? NSNull(),
            "isRead": row["is_read"] ?? NSNull(),
            "isSent": row["is_sent"] ?? NSNull(),
            "isDelivered": row["is_delivered"] ?? NSNull(),
            "isFinished": row["is_finished"] ?? NSNull(),
            "isEmpty": row["is_empty"] ?? NSNull(),
            "isSystemMessage": row["is_system_message"] ?? NSNull(),
            "isServiceMessage": row["is_service_message"] ?? NSNull(),
            "hasAttachments": row["cache_has_attachments"] ?? NSNull(),
            "isAudioMessage": row["is_audio_message"] ?? NSNull(),
            "itemType": row["item_type"] ?? NSNull(),
            "associatedMessageGuid": row["associated_message_guid"] ?? NSNull(),
            "associatedMessageType": row["associated_message_type"] ?? NSNull(),
            "replyToGuid": row["reply_to_guid"] ?? NSNull(),
            "threadOriginatorGuid": row["thread_originator_guid"] ?? NSNull(),
            "isSpam": row["is_spam"] ?? NSNull(),
            "scheduleType": row["schedule_type"] ?? NSNull(),
            "offGrid": row["sent_or_received_off_grid"] ?? NSNull(),
            "text": text,
        ]
        appendLine(f, jsonLine(obj))
        stats.count += 1
    }
    f.closeFile()
    return stats
}


func exportAttachments(_ db: OpaquePointer, _ url: URL) -> Int {
    let f = openWrite(url)
    var count = 0
    let preferred = ["ROWID", "guid", "created_date", "filename", "transfer_name", "uti", "mime_type", "total_bytes", "is_outgoing", "is_sticker", "is_commsafety_sensitive"]
    let sql = selectExisting(db, "attachment", preferred) + " ORDER BY ROWID"
    stream(db, sql) { row in
        let obj: [String: Any] = [
            "rowid": row["ROWID"] ?? NSNull(),
            "guid": row["guid"] ?? NSNull(),
            "createdDateISO": timestampDouble(row, "created_date").flatMap(appleTimestampToISO) ?? NSNull(),
            "filename": row["filename"] ?? NSNull(),
            "transferName": row["transfer_name"] ?? NSNull(),
            "uti": row["uti"] ?? NSNull(),
            "mimeType": row["mime_type"] ?? NSNull(),
            "totalBytes": row["total_bytes"] ?? NSNull(),
            "isOutgoing": row["is_outgoing"] ?? NSNull(),
            "isSticker": row["is_sticker"] ?? NSNull(),
            "isCommsafetySensitive": row["is_commsafety_sensitive"] ?? NSNull(),
        ]
        appendLine(f, jsonLine(obj))
        count += 1
    }
    f.closeFile()
    return count
}

func exportMessageAttachments(_ db: OpaquePointer, _ url: URL) -> Int {
    let f = openWrite(url)
    var count = 0
    let sql = "SELECT maj.message_id, m.guid AS message_guid, maj.attachment_id, a.guid AS attachment_guid FROM message_attachment_join maj JOIN message m ON m.ROWID = maj.message_id JOIN attachment a ON a.ROWID = maj.attachment_id ORDER BY maj.message_id, maj.attachment_id"
    stream(db, sql) { row in
        let obj: [String: Any] = [
            "messageId": row["message_id"] ?? NSNull(),
            "messageGuid": row["message_guid"] ?? NSNull(),
            "attachmentId": row["attachment_id"] ?? NSNull(),
            "attachmentGuid": row["attachment_guid"] ?? NSNull(),
        ]
        appendLine(f, jsonLine(obj))
        count += 1
    }
    f.closeFile()
    return count
}


// Locate the repo root by walking up from the current directory until we find
// the apple-messages-export package (its parent is the repo root).
func findRepoRoot(_ start: URL) -> URL? {
    var dir = start
    while true {
        let marker = dir.appendingPathComponent("apple-messages-export").appendingPathComponent("Package.swift")
        if FileManager.default.fileExists(atPath: marker.path) {
            return dir
        }
        let parent = dir.deletingLastPathComponent()
        if parent.path == dir.path { return nil }
        dir = parent
    }
}

// Default target = <repo-root>/public/upload/data/apple-messages-export/.
// This is an existing operator-owned directory; we never delete or recreate
// public/upload, public/upload/data, or public/upload/media.
func resolveTargetDir(argsOut: URL?) -> URL {
    if let argsOut { return argsOut }
    let cwd = URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
    if let root = findRepoRoot(cwd) {
        return root
            .appendingPathComponent("public")
            .appendingPathComponent("upload")
            .appendingPathComponent("data")
            .appendingPathComponent("apple-messages-export")
    }
    return cwd.appendingPathComponent("public").appendingPathComponent("upload").appendingPathComponent("data").appendingPathComponent("apple-messages-export")
}

// Atomic per-file commit: write a temp file inside the target dir, then rename.
func atomicWrite(_ text: String, to target: URL) throws {
    let tmp = target.appendingPathExtension("tmp")
    try text.write(to: tmp, atomically: true, encoding: .utf8)
    _ = try FileManager.default.replaceItemAt(target, withItemAt: tmp)
}

@main
struct AppleMessagesExport {
    static func main() {
        let args = parseArgs()
        guard FileManager.default.fileExists(atPath: defaultDB.path) else {
            fail("Messages database not found at \(defaultDB.path)")
        }

        let db = openReadOnly(defaultDB)
        defer { sqlite3_close(db) }
        print("Messages DB opened READ-ONLY: \(defaultDB.path)")

        // Timestamp encoding recon (no body text printed). Live DB timestamps are
        // nanoseconds since 2001-01-01 (e.g. 809312575831244416 ns ~= mid-2026).
        var recon: OpaquePointer?
        let now = Date().timeIntervalSince1970
        let reconSQL = "SELECT MAX(date) AS maxdate FROM message"
        if sqlite3_prepare_v2(db, reconSQL, -1, &recon, nil) == SQLITE_OK {
            if sqlite3_step(recon) == SQLITE_ROW {
                let maxRaw = sqlite3_column_double(recon, 0)
                let converted = maxRaw / 1_000_000_000 + APPLE_EPOCH_UNIX_OFFSET
                print("timestamp recon: appleDateRawNs=\(Int(maxRaw)) -> unix=\(Int(converted)) now=\(Int(now)) deltaSeconds=\(Int(converted - now))")
            }
            sqlite3_finalize(recon)
        }

        let targetDir = resolveTargetDir(argsOut: args.outDir)
        do {
            try FileManager.default.createDirectory(at: targetDir, withIntermediateDirectories: true)
        } catch {
            fail("cannot create target directory \(targetDir.path): \(error)")
        }
        print("target=\(targetDir.path)")

        let files = [
            "identities.jsonl",
            "conversations.jsonl",
            "conversation-participants.jsonl",
            "messages.jsonl",
            "attachments.jsonl",
            "message-attachments.jsonl",
        ]

        do {
            let handlesTmp = targetDir.appendingPathComponent("identities.jsonl.tmp")
            let handles = exportHandles(db, handlesTmp, args.maxHandles)
            _ = try FileManager.default.replaceItemAt(targetDir.appendingPathComponent("identities.jsonl"), withItemAt: handlesTmp)

            let chatsTmp = targetDir.appendingPathComponent("conversations.jsonl.tmp")
            let chats = exportChats(db, chatsTmp)
            _ = try FileManager.default.replaceItemAt(targetDir.appendingPathComponent("conversations.jsonl"), withItemAt: chatsTmp)

            let participantsTmp = targetDir.appendingPathComponent("conversation-participants.jsonl.tmp")
            let participants = exportChatParticipants(db, participantsTmp)
            _ = try FileManager.default.replaceItemAt(targetDir.appendingPathComponent("conversation-participants.jsonl"), withItemAt: participantsTmp)

            let messagesTmp = targetDir.appendingPathComponent("messages.jsonl.tmp")
            let msgStats = exportMessages(db, messagesTmp, args.maxMessages)
            _ = try FileManager.default.replaceItemAt(targetDir.appendingPathComponent("messages.jsonl"), withItemAt: messagesTmp)

            let attachmentsTmp = targetDir.appendingPathComponent("attachments.jsonl.tmp")
            let attachments = exportAttachments(db, attachmentsTmp)
            _ = try FileManager.default.replaceItemAt(targetDir.appendingPathComponent("attachments.jsonl"), withItemAt: attachmentsTmp)

            let messageAttachmentsTmp = targetDir.appendingPathComponent("message-attachments.jsonl.tmp")
            let messageAttachments = exportMessageAttachments(db, messageAttachmentsTmp)
            _ = try FileManager.default.replaceItemAt(targetDir.appendingPathComponent("message-attachments.jsonl"), withItemAt: messageAttachmentsTmp)

            let timestampFormatter = ISO8601DateFormatter()
            timestampFormatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
            let counts: [String: Any] = [
                "handles": handles,
                "chats": chats,
                "chatParticipants": participants,
                "messages": msgStats.count,
                "messagesWithText": msgStats.withText,
                "attachments": attachments,
                "messageAttachments": messageAttachments,
            ]
            let manifest: [String: Any] = [
                "exportVersion": exportVersion,
                "sourceSystem": sourceSystem,
                "generatedAt": timestampFormatter.string(from: Date()),
                "databasePath": defaultDB.path,
                "databaseReadOnly": true,
                "timestampEncoding": "apple_nanoseconds_since_2001_div_1e9_plus_\(Int(APPLE_EPOCH_UNIX_OFFSET))",
                "counts": counts,
                "minimumMessageDate": msgStats.minISO ?? NSNull(),
                "maximumMessageDate": msgStats.maxISO ?? NSNull(),
                "files": files,
            ]
            try atomicWrite(jsonLine(manifest), to: targetDir.appendingPathComponent("manifest.json"))

            print("EXPORT SUCCESS")
            print("handles=\(handles) chats=\(chats) participants=\(participants) messages=\(msgStats.count) (withText=\(msgStats.withText)) attachments=\(attachments) messageAttachments=\(messageAttachments)")
            print("min=\(msgStats.minISO ?? "?") max=\(msgStats.maxISO ?? "?")")
            print("output=\(targetDir.path)")
        } catch {
            fail("export failed: \(error)")
        }
    }
}

