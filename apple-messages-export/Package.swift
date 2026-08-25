// swift-tools-version: 6.3
import PackageDescription

// CulebraLuxe — Apple Messages full-fidelity LOCAL exporter.
// Reads ~/Library/Messages/chat.db READ-ONLY and streams a durable, versioned
// export package (manifest.json + JSONL). Never writes to Apple's DB, never
// extracts attachment binaries, never logs message bodies.
let package = Package(
    name: "apple-messages-export",
    targets: [
        .executableTarget(name: "apple-messages-export"),
    ],
    swiftLanguageModes: [.v6]
)
