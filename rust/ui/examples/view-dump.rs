// Print the markup this crate renders, for looking at by eye.
//
// Not a test: a way to SEE the output. Every blank-page report so far has been diagnosed by reasoning about a browser
// nobody could look at, and reasoning got it wrong repeatedly. This prints exactly what `view::render` produces.
//
//   cargo run -p ui --example view-dump -- site-home
//   curl -s '.../api/rust-ui/public-page?screen=site-home' | \
//     cargo run -p ui --example view-dump -- site-home --page
use std::io::Read;

use ui::model::{screen, Model, PageContent, Row};
use ui::render;

fn rows(pairs: &[(&str, &str, &str)]) -> Vec<Row> {
    pairs
        .iter()
        .map(|(id, label, value)| Row {
            id: (*id).to_string(),
            cells: vec![(*label).to_string(), (*value).to_string()],
            badge: None,
        })
        .collect()
}

fn main() {
    let mut args = std::env::args().skip(1);
    let key = args.next().unwrap_or_else(|| "site-home".into());
    let page_from_stdin = args.any(|arg| arg == "--page");
    let Some(target) = screen(&key) else {
        eprintln!("unknown screen: {key}");
        std::process::exit(1);
    };

    // THE REAL PAYLOAD when it is piped in, so what is printed is what the browser is handed rather than what this
    // sample guessed.
    let page = if page_from_stdin {
        let mut payload = String::new();
        std::io::stdin()
            .read_to_string(&mut payload)
            .expect("payload");
        match serde_json::from_str::<PageContent>(&payload) {
            Ok(page) => {
                eprintln!(
                    "parsed: hero title {} chars, buyers title {} chars, listings {}",
                    page.hero.title.chars().count(),
                    page.buyers.title.chars().count(),
                    page.listings.len()
                );
                Some(page)
            }
            Err(error) => {
                eprintln!("could not read the payload: {error}");
                std::process::exit(1);
            }
        }
    } else {
        None
    };

    let model = Model {
        screen: target,
        rows: match key.as_str() {
            "site-properties" => rows(&[
                ("villa-del-mar", "Villa del Mar", "$2,400,000"),
                ("casa-azul", "Casa Azul", "Price upon request"),
            ]),
            _ => Vec::new(),
        },
        page,
        ..Model::default()
    };

    println!("{}", render(&model));
}
