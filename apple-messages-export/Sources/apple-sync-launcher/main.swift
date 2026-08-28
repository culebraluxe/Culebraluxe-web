import Foundation
import SQLite3

// ===========================================================================
// CulebraLuxe — FDA-grantable launcher for the Apple Messages production sync.
//
// macOS launchd (com.culebraluxe.apple-sync) invokes THIS compiled binary. The
// operator grants Full Disk Access to this exact binary path in System Settings
// (the one-time privacy step); TCC responsibility then propagates to the bash
// script and Swift exporter it spawns, which is what lets the chain read
// ~/Library/Messages/chat.db. A compiled binary is used because macOS can only
// grant Full Disk Access to executables/apps — NOT to /bin/bash.
//
// Usage:
//   apple-sync-launcher                        run the canonical sync script
//   apple-sync-launcher --verify-tcc           probe chat.db READ-ONLY access
//
// Env (set by the LaunchAgent plist):
//   CULEBRALUXE_REPO              repository root
//   CULEBRALUXE_APPLE_SYNC_SCRIPT canonical sync script (deployed copy)
// ===========================================================================

@main
struct AppleSyncLauncher {
    static func main() {
        let args = Array(CommandLine.arguments.dropFirst())
        if args.contains("--verify-tcc") {
            verifyTcc()
            return
        }

        let env = ProcessInfo.processInfo.environment
        let repo = env["CULEBRALUXE_REPO"] ?? "\(NSHomeDirectory())/Documents/Culebraluxe-web"
        let script = env["CULEBRALUXE_APPLE_SYNC_SCRIPT"] ?? "\(repo)/scripts/apple-sync.sh"

        print("apple-sync-launcher: repo=\(repo)")
        print("apple-sync-launcher: script=\(script)")

        guard FileManager.default.fileExists(atPath: script) else {
            FileHandle.standardError.write(Data("apple-sync-launcher: script not found: \(script)\n".utf8))
            exit(EXIT_FAILURE)
        }

        let p = Process()
        p.executableURL = URL(fileURLWithPath: "/bin/bash")
        p.arguments = [script]
        var childEnv = env
        childEnv["CULEBRALUXE_REPO"] = repo
        childEnv["CULEBRALUXE_APPLE_SYNC_SCRIPT"] = script
        if (childEnv["PATH"] ?? "").isEmpty {
            childEnv["PATH"] = "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"
        }
        p.environment = childEnv

        do {
            try p.run()
            p.waitUntilExit()
            exit(Int32(p.terminationStatus))
        } catch {
            FileHandle.standardError.write(Data("apple-sync-launcher: failed to run: \(error)\n".utf8))
            exit(EXIT_FAILURE)
        }
    }

    /// Proves the exact Full-Disk-Access boundary for the binary launchd runs:
    /// can we open ~/Library/Messages/chat.db READ-ONLY?
    static func verifyTcc() {
        let dbPath = NSHomeDirectory() + "/Library/Messages/chat.db"
        guard FileManager.default.fileExists(atPath: dbPath) else {
            print("TCC VERIFY: chat.db not found at \(dbPath)")
            exit(EXIT_FAILURE)
        }
        var db: OpaquePointer?
        let rc = sqlite3_open_v2(dbPath, &db, SQLITE_OPEN_READONLY, nil)
        if rc == SQLITE_OK, let db {
            var stmt: OpaquePointer?
            var count: Int64 = -1
            let qrc = sqlite3_prepare_v2(db, "SELECT COUNT(*) FROM message;", -1, &stmt, nil)
            if qrc == SQLITE_OK {
                if sqlite3_step(stmt) == SQLITE_ROW {
                    count = sqlite3_column_int64(stmt, 0)
                }
                sqlite3_finalize(stmt)
            }
            sqlite3_close(db)
            print("TCC VERIFY: OK chat.db opened READ-ONLY message_count=\(count)")
            exit(EXIT_SUCCESS)
        } else {
            let msg = db.map { String(cString: sqlite3_errmsg($0)) } ?? "unknown"
            sqlite3_close(db)
            print("TCC VERIFY: DENIED chat.db not readable: \(msg)")
            exit(EXIT_FAILURE)
        }
    }
}
