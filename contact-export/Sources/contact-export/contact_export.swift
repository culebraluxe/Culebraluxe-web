import Contacts
import Foundation

struct LabeledTextValue: Codable {
    let sourceLabel: String?
    let value: String
}

struct PostalAddressValue: Codable {
    let sourceLabel: String?
    let street: String
    let city: String
    let state: String
    let postalCode: String
    let country: String
    let isoCountryCode: String
}

struct ExportContact: Codable {
    let sourceId: String

    let namePrefix: String
    let givenName: String
    let middleName: String
    let familyName: String
    let nameSuffix: String
    let nickname: String

    let organization: String
    let department: String
    let jobTitle: String

    let emails: [LabeledTextValue]
    let phones: [LabeledTextValue]
    let postalAddresses: [PostalAddressValue]
}

struct ContactExportBatch: Codable {
    let schemaVersion: Int
    let sourceSystem: String
    let exportId: String
    let exportedAt: String
    let contacts: [ExportContact]
}

enum ContactExportError: Error, CustomStringConvertible {
    case invalidLimit
    case accessDenied

    var description: String {
        switch self {
        case .invalidLimit:
            return "--limit must be followed by a positive integer."
        case .accessDenied:
            return "Contacts access denied."
        }
    }
}

@main
struct ContactExport {
    static func main() async {
        do {
            let limit = try parseLimit()
            let store = CNContactStore()
            let granted = try await store.requestAccess(for: .contacts)

            guard granted else {
                throw ContactExportError.accessDenied
            }

            let keys: [CNKeyDescriptor] = [
                CNContactIdentifierKey as CNKeyDescriptor,

                CNContactNamePrefixKey as CNKeyDescriptor,
                CNContactGivenNameKey as CNKeyDescriptor,
                CNContactMiddleNameKey as CNKeyDescriptor,
                CNContactFamilyNameKey as CNKeyDescriptor,
                CNContactNameSuffixKey as CNKeyDescriptor,
                CNContactNicknameKey as CNKeyDescriptor,

                CNContactOrganizationNameKey as CNKeyDescriptor,
                CNContactDepartmentNameKey as CNKeyDescriptor,
                CNContactJobTitleKey as CNKeyDescriptor,

                CNContactEmailAddressesKey as CNKeyDescriptor,
                CNContactPhoneNumbersKey as CNKeyDescriptor,
                CNContactPostalAddressesKey as CNKeyDescriptor
            ]

            let request = CNContactFetchRequest(keysToFetch: keys)
            var contacts: [ExportContact] = []

            try store.enumerateContacts(with: request) { contact, stop in
                contacts.append(
                    ExportContact(
                        sourceId: contact.identifier,

                        namePrefix: contact.namePrefix,
                        givenName: contact.givenName,
                        middleName: contact.middleName,
                        familyName: contact.familyName,
                        nameSuffix: contact.nameSuffix,
                        nickname: contact.nickname,

                        organization: contact.organizationName,
                        department: contact.departmentName,
                        jobTitle: contact.jobTitle,

                        emails: contact.emailAddresses.map {
                            LabeledTextValue(
                                sourceLabel: $0.label,
                                value: $0.value as String
                            )
                        },

                        phones: contact.phoneNumbers.map {
                            LabeledTextValue(
                                sourceLabel: $0.label,
                                value: $0.value.stringValue
                            )
                        },

                        postalAddresses: contact.postalAddresses.map {
                            let address = $0.value

                            return PostalAddressValue(
                                sourceLabel: $0.label,
                                street: address.street,
                                city: address.city,
                                state: address.state,
                                postalCode: address.postalCode,
                                country: address.country,
                                isoCountryCode: address.isoCountryCode
                            )
                        }
                    )
                )

                if let limit, contacts.count >= limit {
                    stop.pointee = true
                }
            }

            let timestampFormatter = ISO8601DateFormatter()
            timestampFormatter.formatOptions = [
                .withInternetDateTime,
                .withFractionalSeconds
            ]

            let batch = ContactExportBatch(
                schemaVersion: 1,
                sourceSystem: "apple_contacts",
                exportId: UUID().uuidString,
                exportedAt: timestampFormatter.string(from: Date()),
                contacts: contacts
            )

            let encoder = JSONEncoder()
            encoder.outputFormatting = [
                .prettyPrinted,
                .sortedKeys
            ]

            let data = try encoder.encode(batch)

            FileHandle.standardOutput.write(data)
            FileHandle.standardOutput.write(Data([0x0A]))
        } catch {
            writeError("Contacts export error: \(error)")
            exit(EXIT_FAILURE)
        }
    }

    private static func parseLimit() throws -> Int? {
        let arguments = Array(CommandLine.arguments.dropFirst())

        guard let limitIndex = arguments.firstIndex(of: "--limit") else {
            return nil
        }

        let valueIndex = arguments.index(after: limitIndex)

        guard
            valueIndex < arguments.endIndex,
            let value = Int(arguments[valueIndex]),
            value > 0
        else {
            throw ContactExportError.invalidLimit
        }

        return value
    }

    private static func writeError(_ message: String) {
        guard let data = "\(message)\n".data(using: .utf8) else {
            return
        }

        FileHandle.standardError.write(data)
    }
}