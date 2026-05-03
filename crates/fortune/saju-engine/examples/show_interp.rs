//! Quick visual check of the new interpretation field. Run:
//!   cargo run -p saju-engine --example show_interp
use saju_engine::SajuEngine;
use serde_json::json;

fn main() {
    let engine = SajuEngine;
    let input = json!({
        "birth_date": "1991-11-09",
        "birth_time": "12:00",
        "gender": "male",
        "calendar_type": "solar",
    });
    let (result, _v) = engine.generate("saju", &input);
    let interp = &result["interpretation"];
    println!("HEADLINE: {}\n", interp["headline"].as_str().unwrap_or("?"));
    for s in interp["sections"].as_array().unwrap() {
        println!("=== {} ({}) ===", s["title"].as_str().unwrap_or("?"), s["key"].as_str().unwrap_or("?"));
        println!("{}\n", s["body"].as_str().unwrap_or("?"));
    }
}
