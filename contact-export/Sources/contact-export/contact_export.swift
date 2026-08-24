import Contacts
import Foundation

@main
struct ContactExport {
    static func main() async {
        let store = CNContactStore()

        do {
            let granted = try await store.requestAccess(for: .contacts)

            if granted {
                let keys: [CNKeyDescriptor] = [
                    CNContactGivenNameKey as CNKeyDescriptor,
                    CNContactFamilyNameKey as CNKeyDescriptor
                ]

                let request = CNContactFetchRequest(keysToFetch: keys)

                var count = 0

                try store.enumerateContacts(with: request) { contact, stop in
                    print("\(contact.givenName) \(contact.familyName)")
                    count += 1

                    if count >= 5 {
                        stop.pointee = true
                    }
                }
            } else {
                print("Contacts access denied.")
            }
        } catch {
            print("Contacts access error: \(error)")
        }
    }
}