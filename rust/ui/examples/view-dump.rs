// Print the markup this crate renders, for looking at by eye.
//
// Not a test: a way to SEE the output. Every blank-page report so far has been diagnosed by reasoning about a browser
// nobody could look at, and reasoning got it wrong three times in a row. This prints exactly what `view::render`
// produces for a screen, so the markup can be read as markup.
//
//   cargo run -p ui --example view-dump -- site-home
use ui::model::{screen, Model, Row};
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
    let key = std::env::args().nth(1).unwrap_or_else(|| "site-home".into());
    let Some(target) = screen(&key) else {
        eprintln!("unknown screen: {key}");
        std::process::exit(1);
    };

    let model = Model {
        screen: target,
        rows: match key.as_str() {
            "site-home" => rows(&[
                ("home.hero:title", "Culebra · Puerto Rico", "An island held quietly between sea and sky."),
                ("home.hero:body", "Body", "A curated portfolio of architectural residences and beachfront estates, presented with the discretion the island deserves."),
                ("home.hero:cta", "Call to action", "View the Collection → #properties"),
            ]),
            "site-properties" => rows(&[
                ("villa-del-mar", "Villa del Mar", "$2,400,000"),
                ("casa-azul", "Casa Azul", "Price upon request"),
            ]),
            _ => Vec::new(),
        },
        ..Model::default()
    };

    println!("{}", render(&model));
}
