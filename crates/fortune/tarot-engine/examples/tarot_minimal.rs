//! Draw a three-card past/present/future spread and print the JSON.
use serde_json::json;
use tarot_engine::TarotEngine;

fn main() {
    let engine = TarotEngine;
    let (reading, version) = engine.generate(
        "tarot_three",
        &json!({
            "birth_date": "1990-05-15",
            "birth_time": "14:30",
            "calendar_type": "solar"
        }),
    );
    println!("version={version}");
    println!("{}", serde_json::to_string_pretty(&reading).unwrap());
}
