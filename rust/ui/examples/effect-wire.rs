use ui::{Effect, Msg};
fn main() {
    println!("rows  -> {}", serde_json::to_string(&Effect::FetchRows { screen: "clients", scope: None }).unwrap());
    println!("page  -> {}", serde_json::to_string(&Effect::FetchPage { screen: "site-home" }).unwrap());
    println!("mount -> {}", serde_json::to_string(&vec![Effect::FetchPage { screen: "site-home" }]).unwrap());
    let _ = Msg::ScreenOpened;
}
