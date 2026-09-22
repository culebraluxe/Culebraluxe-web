// Prints the effect wire format, which is the contract with `lib/rust-ui/boot.ts`.
//
// Run it when an effect is added or a field moves: `boot.ts` switches on the exact tag string and reads the field
// names off the JSON, so a rename here is a request the host silently ignores — which is what "every screen is blank"
// looked like. `Msgs` are not serialised (the host sends them as its own calls), so only `Effect` appears here.

use ui::Effect;

fn main() {
    println!("rows   -> {}", serde_json::to_string(&Effect::FetchRows { screen: "clients", scope: None }).unwrap());
    println!("page   -> {}", serde_json::to_string(&Effect::FetchPage { screen: "site-home", scope: None }).unwrap());
    println!("mount  -> {}", serde_json::to_string(&vec![Effect::FetchPage { screen: "site-home", scope: None }]).unwrap());
    println!("record -> {}", serde_json::to_string(&Effect::FetchPage { screen: "site-property-detail", scope: Some("villa-rosada".into()) }).unwrap());
}
