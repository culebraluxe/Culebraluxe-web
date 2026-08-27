import Foundation
import SQLite3

let home = FileManager.default.homeDirectoryForCurrentUser

let dbURL = home
    .appendingPathComponent("Library")
    .appendingPathComponent("Messages")
    .appendingPathComponent("chat.db")

let outputURL = URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
    .appendingPathComponent("messages_schema_recon.txt")

print("Opening:")
print(dbURL.path)

var db: OpaquePointer?

let result = sqlite3_open_v2(
    dbURL.path,
    &db,
    SQLITE_OPEN_READONLY,
    nil
)

guard result == SQLITE_OK, let db else {
    if let db {
        print("SQLite error:", String(cString: sqlite3_errmsg(db)))
        sqlite3_close(db)
    }

    exit(1)
}

defer {
    sqlite3_close(db)
}

func queryStrings(_ sql: String) -> [[String]] {
    var statement: OpaquePointer?

    guard sqlite3_prepare_v2(db, sql, -1, &statement, nil) == SQLITE_OK else {
        print("Prepare error:", String(cString: sqlite3_errmsg(db)))
        return []
    }

    defer {
        sqlite3_finalize(statement)
    }

    var rows: [[String]] = []

    while sqlite3_step(statement) == SQLITE_ROW {
        var row: [String] = []

        for index in 0..<sqlite3_column_count(statement) {
            if sqlite3_column_type(statement, index) == SQLITE_NULL {
                row.append("NULL")
            } else if let text = sqlite3_column_text(statement, index) {
                row.append(String(cString: text))
            } else {
                row.append("")
            }
        }

        rows.append(row)
    }

    return rows
}

let tables = queryStrings("""
SELECT name
FROM sqlite_master
WHERE type = 'table'
ORDER BY name;
""")
.compactMap { $0.first }

var output = """

APPLE MESSAGES DATABASE RECONNAISSANCE
======================================

Database:
\(dbURL.path)

Generated:
\(Date())

Mode:
READ ONLY

"""

for table in tables {

    output += """

    ============================================================
    TABLE: \(table)
    ============================================================

    """

    // Row count

    let escapedTable = table.replacingOccurrences(of: "\"", with: "\"\"")

    let counts = queryStrings("""
    SELECT COUNT(*)
    FROM "\(escapedTable)";
    """)

    if let count = counts.first?.first {
        output += "ROW COUNT: \(count)\n\n"
    }

    // Columns

    output += "COLUMNS\n-------\n"

    let columns = queryStrings("""
    PRAGMA table_info("\(escapedTable)");
    """)

    for column in columns {

        // cid, name, type, notnull, default, pk

        let cid = column.indices.contains(0) ? column[0] : ""
        let name = column.indices.contains(1) ? column[1] : ""
        let type = column.indices.contains(2) ? column[2] : ""
        let notNull = column.indices.contains(3) ? column[3] : ""
        let defaultValue = column.indices.contains(4) ? column[4] : ""
        let pk = column.indices.contains(5) ? column[5] : ""

        output += """
        cid=\(cid)
          name=\(name)
          type=\(type)
          notnull=\(notNull)
          default=\(defaultValue)
          primaryKey=\(pk)

        """
    }

    // Foreign keys

    output += "\nFOREIGN KEYS\n------------\n"

    let foreignKeys = queryStrings("""
    PRAGMA foreign_key_list("\(escapedTable)");
    """)

    if foreignKeys.isEmpty {
        output += "(none)\n"
    } else {
        for fk in foreignKeys {
            output += fk.joined(separator: " | ") + "\n"
        }
    }

    // Indexes

    output += "\nINDEXES\n-------\n"

    let indexes = queryStrings("""
    PRAGMA index_list("\(escapedTable)");
    """)

    if indexes.isEmpty {
        output += "(none)\n"
    } else {
        for index in indexes {
            output += index.joined(separator: " | ") + "\n"
        }
    }

    // CREATE SQL

    output += "\nCREATE SQL\n----------\n"

    let createSQL = queryStrings("""
    SELECT sql
    FROM sqlite_master
    WHERE type = 'table'
      AND name = '\(table.replacingOccurrences(of: "'", with: "''"))';
    """)

    if let sql = createSQL.first?.first {
        output += sql + "\n"
    }
}

do {
    try output.write(
        to: outputURL,
        atomically: true,
        encoding: .utf8
    )

    print("")
    print("SUCCESS")
    print("Reconnaissance written to:")
    print(outputURL.path)

} catch {
    print("Failed writing output:")
    print(error)
    exit(1)
}