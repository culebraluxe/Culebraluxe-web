import Foundation
import SQLite3

// Read-only exporter for Apple's local CallHistory Core Data store.
// Emits ALL call rows without guessing Apple enum values. The TypeScript intake
// classifies FaceTime vs normal calls from the raw provider/type values.

let home = FileManager.default.homeDirectoryForCurrentUser
let candidates = [
    home.appendingPathComponent("Library/Application Support/CallHistoryDB/CallHistory.storedata"),
    home.appendingPathComponent("Library/Application Support/CallHistoryDB/CallHistory.db"),
]

func fail(_ message: String) -> Never {
    FileHandle.standardError.write(Data(("ERROR: \(message)\n").utf8))
    exit(EXIT_FAILURE)
}

func openReadOnly(_ url: URL) -> OpaquePointer {
    var db: OpaquePointer?
    let rc = sqlite3_open_v2(url.path, &db, SQLITE_OPEN_READONLY, nil)
    guard rc == SQLITE_OK, let db else {
        let msg = db.map { String(cString: sqlite3_errmsg($0)) } ?? "unknown"
        sqlite3_close(db)
        fail("cannot open CallHistory DB read-only: \(msg)")
    }
    return db
}

func columns(_ db: OpaquePointer, table: String) -> Set<String> {
    var stmt: OpaquePointer?
    guard sqlite3_prepare_v2(db, "PRAGMA table_info(\"\(table)\")", -1, &stmt, nil) == SQLITE_OK else { return [] }
    defer { sqlite3_finalize(stmt) }
    var result = Set<String>()
    while sqlite3_step(stmt) == SQLITE_ROW {
        if let raw = sqlite3_column_text(stmt, 1) { result.insert(String(cString: raw)) }
    }
    return result
}

func pick(_ available: Set<String>, _ names: [String]) -> String? {
    names.first { available.contains($0) }
}

func sqlExpr(_ column: String?, alias: String) -> String {
    column.map { "\"\($0)\" AS \"\(alias)\"" } ?? "NULL AS \"\(alias)\""
}

func readValue(_ stmt: OpaquePointer?, _ index: Int32) -> Any {
    switch sqlite3_column_type(stmt, index) {
    case SQLITE_INTEGER: return sqlite3_column_int64(stmt, index)
    case SQLITE_FLOAT: return sqlite3_column_double(stmt, index)
    case SQLITE_TEXT: return String(cString: sqlite3_column_text(stmt, index))
    default: return NSNull()
    }
}

let appleEpoch: Double = 978_307_200
func callDateISO(_ raw: Any) -> Any {
    let value: Double?
    if let v = raw as? Double { value = v }
    else if let v = raw as? Int64 { value = Double(v) }
    else { value = nil }
    guard let value else { return NSNull() }
    // CallHistory/CoreData normally stores seconds since 2001. Be tolerant of
    // nanosecond representations without silently changing source data.
    let seconds = value > 1_000_000_000_000 ? value / 1_000_000_000 : value
    let unix = seconds + appleEpoch
    guard unix > 0 else { return NSNull() }
    let formatter = ISO8601DateFormatter()
    formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
    return formatter.string(from: Date(timeIntervalSince1970: unix))
}

let args = Array(CommandLine.arguments.dropFirst())
var outPath = "public/upload/data/apple-messages-export/calls.jsonl"
if let i = args.firstIndex(of: "--out"), i + 1 < args.count { outPath = args[i + 1] }
let outURL = URL(fileURLWithPath: outPath)
let schemaURL = outURL.deletingLastPathComponent().appendingPathComponent("call-history-schema.json")

try? FileManager.default.createDirectory(at: outURL.deletingLastPathComponent(), withIntermediateDirectories: true)

guard let dbURL = candidates.first(where: { FileManager.default.fileExists(atPath: $0.path) }) else {
    fail("CallHistory database not found. Checked: \(candidates.map(\.path).joined(separator: ", "))")
}
let db = openReadOnly(dbURL)
defer { sqlite3_close(db) }

let table = "ZCALLRECORD"
let available = columns(db, table: table)
guard !available.isEmpty else { fail("\(table) not found or has no readable columns in \(dbURL.path)") }

let idCol = pick(available, ["ZUNIQUE_ID", "ZUUID", "ZCALLUUID"])
let addressCol = pick(available, ["ZADDRESS", "ZREMOTEADDRESS", "ZHANDLE"])
let dateCol = pick(available, ["ZDATE", "ZSTARTDATE"])
let durationCol = pick(available, ["ZDURATION"])
let originatedCol = pick(available, ["ZORIGINATED", "ZOUTGOING"])
let answeredCol = pick(available, ["ZANSWERED"])
let typeCol = pick(available, ["ZCALLTYPE", "ZCALLTYPE"])
let providerCol = pick(available, ["ZSERVICE_PROVIDER", "ZSERVICEPROVIDER", "ZPROVIDER"])
let countryCol = pick(available, ["ZISO_COUNTRY_CODE", "ZISOCOUNTRYCODE"])

let selected = [
    "Z_PK AS rowid",
    sqlExpr(idCol, alias: "unique_id"),
    sqlExpr(addressCol, alias: "address"),
    sqlExpr(dateCol, alias: "date_raw"),
    sqlExpr(durationCol, alias: "duration"),
    sqlExpr(originatedCol, alias: "originated"),
    sqlExpr(answeredCol, alias: "answered"),
    sqlExpr(typeCol, alias: "call_type"),
    sqlExpr(providerCol, alias: "service_provider"),
    sqlExpr(countryCol, alias: "country_code"),
].joined(separator: ", ")

let sql = "SELECT \(selected) FROM \(table) ORDER BY Z_PK"
var stmt: OpaquePointer?
guard sqlite3_prepare_v2(db, sql, -1, &stmt, nil) == SQLITE_OK else {
    fail("CallHistory query prepare failed: \(String(cString: sqlite3_errmsg(db)))")
}
defer { sqlite3_finalize(stmt) }

FileManager.default.createFile(atPath: outURL.path, contents: nil)
guard let out = FileHandle(forWritingAtPath: outURL.path) else { fail("cannot open output \(outURL.path)") }
defer { out.closeFile() }

var count = 0
while sqlite3_step(stmt) == SQLITE_ROW {
    var row: [String: Any] = [:]
    let names = ["rowid", "uniqueId", "address", "dateRaw", "duration", "originated", "answered", "callType", "serviceProvider", "countryCode"]
    for i in 0..<sqlite3_column_count(stmt) { row[names[Int(i)]] = readValue(stmt, i) }
    row["dateISO"] = callDateISO(row["dateRaw"] ?? NSNull())
    if row["uniqueId"] is NSNull { row["uniqueId"] = String(describing: row["rowid"] ?? "") }
    let data = try JSONSerialization.data(withJSONObject: row, options: [.sortedKeys])
    out.write(data); out.write(Data("\n".utf8)); count += 1
}

let schema: [String: Any] = [
    "databasePath": dbURL.path,
    "databaseReadOnly": true,
    "table": table,
    "availableColumns": available.sorted(),
    "mappedColumns": [
        "uniqueId": idCol ?? NSNull(), "address": addressCol ?? NSNull(), "date": dateCol ?? NSNull(),
        "duration": durationCol ?? NSNull(), "originated": originatedCol ?? NSNull(), "answered": answeredCol ?? NSNull(),
        "callType": typeCol ?? NSNull(), "serviceProvider": providerCol ?? NSNull(), "countryCode": countryCol ?? NSNull(),
    ],
    "rows": count,
]
let schemaData = try JSONSerialization.data(withJSONObject: schema, options: [.prettyPrinted, .sortedKeys])
try schemaData.write(to: schemaURL)

print("CALL HISTORY EXPORT SUCCESS")
print("database=\(dbURL.path)")
print("rows=\(count)")
print("output=\(outURL.path)")
print("schema=\(schemaURL.path)")
