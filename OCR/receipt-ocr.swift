import Foundation
import Vision

struct ReceiptDataset: Codable {
    let sourceFile: String
    let merchantCandidate: String?
    let transactionDate: String?
    let total: Decimal?
    let phone: String?
    let paymentMethod: String?
    let rawText: String
    let lines: [String]
}

enum ReceiptOCRError: Error, CustomStringConvertible {
    case usage
    case noText

    var description: String {
        switch self {
        case .usage:
            return "Usage: receipt-ocr <receipt-image>"
        case .noText:
            return "Apple Vision did not recognize any text in the image."
        }
    }
}

@main
struct ReceiptOCRCLI {
    static func main() async {
        do {
            guard CommandLine.arguments.count >= 2 else {
                throw ReceiptOCRError.usage
            }

            let imagePath = CommandLine.arguments[1]
            let imageURL = URL(fileURLWithPath: imagePath)
            let imageData = try Data(contentsOf: imageURL)

            var request = RecognizeTextRequest()
            request.automaticallyDetectsLanguage = true
            request.usesLanguageCorrection = true

            let observations = try await request.perform(on: imageData)

            let lines = observations
                .map { $0.transcript.trimmingCharacters(in: .whitespacesAndNewlines) }
                .filter { !$0.isEmpty }

            guard !lines.isEmpty else {
                throw ReceiptOCRError.noText
            }

            let dataset = buildDataset(sourceFile: imageURL.lastPathComponent,
                                       lines: lines)

            let encoder = JSONEncoder()
            encoder.outputFormatting = [.prettyPrinted, .sortedKeys, .withoutEscapingSlashes]

            let json = try encoder.encode(dataset)
            FileHandle.standardOutput.write(json)
            FileHandle.standardOutput.write(Data("\n".utf8))

        } catch {
            fputs("receipt-ocr: \(error)\n", stderr)
            exit(1)
        }
    }

    static func buildDataset(sourceFile: String, lines: [String]) -> ReceiptDataset {
        let rawText = lines.joined(separator: "\n")

        return ReceiptDataset(
            sourceFile: sourceFile,
            merchantCandidate: extractMerchantCandidate(from: lines),
            transactionDate: extractDate(from: rawText),
            total: extractTotal(from: rawText),
            phone: extractPhone(from: rawText),
            paymentMethod: extractPaymentMethod(from: lines),
            rawText: rawText,
            lines: lines
        )
    }

    static func extractMerchantCandidate(from lines: [String]) -> String? {
        var candidates: [String] = []

        for line in lines.prefix(8) {
            let upper = line.uppercased()

            if containsDate(line)
                || extractPhone(from: line) != nil
                || upper.hasPrefix("REG")
                || upper.hasPrefix("MC#")
                || upper.hasPrefix("CASHIER")
                || upper.hasPrefix("RECEIPT") {
                break
            }

            guard line.rangeOfCharacter(from: .letters) != nil else { continue }

            candidates.append(line)

            if candidates.count == 2 {
                break
            }
        }

        guard !candidates.isEmpty else { return nil }
        return candidates.joined(separator: " ")
    }

    static func extractDate(from text: String) -> String? {
        let pattern = #"(?<!\d)(\d{2})[-/](\d{2})[-/](\d{4})(?!\d)"#
        guard let groups = firstRegexGroups(pattern: pattern, in: text),
              groups.count >= 4 else {
            return nil
        }

        return "\(groups[3])-\(groups[1])-\(groups[2])"
    }

    static func containsDate(_ text: String) -> Bool {
        extractDate(from: text) != nil
    }

    static func extractPhone(from text: String) -> String? {
        let pattern = #"(?:\+?1[\s.\-]?)?\(?(\d{3})\)?[\s.\-]?(\d{3})[\s.\-]?(\d{4})"#
        guard let groups = firstRegexGroups(pattern: pattern, in: text),
              groups.count >= 4 else {
            return nil
        }

        return "\(groups[1])-\(groups[2])-\(groups[3])"
    }

    static func extractPaymentMethod(from lines: [String]) -> String? {
        let known = [
            "ATH",
            "VISA",
            "MASTERCARD",
            "AMEX",
            "AMERICAN EXPRESS",
            "CASH",
            "EFECTIVO"
        ]

        for line in lines {
            let value = line
                .trimmingCharacters(in: .whitespacesAndNewlines)
                .uppercased()

            if known.contains(value) {
                return value
            }
        }

        return nil
    }

    static func extractTotal(from text: String) -> Decimal? {
        let totalPattern = #"(?im)^\s*(?:TOTAL|TL)\b[^\n$0-9-]*\$?\s*(-?\d{1,9}\.\d{2})\s*$"#
        if let groups = firstRegexGroups(pattern: totalPattern, in: text),
           groups.count >= 2,
           let value = Decimal(string: groups[1]) {
            return value
        }

        let amountPattern = #"\$?\s*(-?\d{1,9}\.\d{2})\b"#
        let values = allRegexGroups(pattern: amountPattern, in: text)
            .compactMap { groups -> Decimal? in
                guard groups.count >= 2 else { return nil }
                return Decimal(string: groups[1])
            }

        return values.max()
    }

    static func firstRegexGroups(pattern: String, in text: String) -> [String]? {
        guard let regex = try? NSRegularExpression(pattern: pattern) else {
            return nil
        }

        let range = NSRange(text.startIndex..<text.endIndex, in: text)

        guard let match = regex.firstMatch(in: text, range: range) else {
            return nil
        }

        return captureGroups(match: match, text: text)
    }

    static func allRegexGroups(pattern: String, in text: String) -> [[String]] {
        guard let regex = try? NSRegularExpression(pattern: pattern) else {
            return []
        }

        let range = NSRange(text.startIndex..<text.endIndex, in: text)

        return regex.matches(in: text, range: range).map {
            captureGroups(match: $0, text: text)
        }
    }

    static func captureGroups(match: NSTextCheckingResult, text: String) -> [String] {
        (0..<match.numberOfRanges).map { index in
            let nsRange = match.range(at: index)

            guard nsRange.location != NSNotFound,
                  let range = Range(nsRange, in: text) else {
                return ""
            }

            return String(text[range])
        }
    }
}
