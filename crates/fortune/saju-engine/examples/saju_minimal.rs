//! Generate a one-day fortune and print the JSON envelope.
use saju_engine::SajuEngine;
use serde_json::json;

fn main() {
    let engine = SajuEngine;
    let (reading, version) = engine.generate(
        "daily",
        &json!({
            "birth_date": "1990-05-15",
            "birth_time": "14:30",
            "calendar_type": "solar"
        }),
    );
    println!("version={version}");
    println!("{}", serde_json::to_string_pretty(&reading).unwrap());
}
