pub fn estimate_claude_cost(model: &str, input_tokens: u32, output_tokens: u32) -> f64 {
    let (input_rate, output_rate) = match model {
        m if m.contains("haiku") => (0.25, 1.25),
        m if m.contains("sonnet") => (3.0, 15.0),
        m if m.contains("opus") => (15.0, 75.0),
        _ => (3.0, 15.0), // fallback to sonnet
    };
    (input_tokens as f64 * input_rate + output_tokens as f64 * output_rate) / 1_000_000.0
}

pub fn estimate_codex_cost(input_tokens: u32, output_tokens: u32) -> f64 {
    const CODEX_INPUT_USD_PER_1M: f64 = 2.50;
    const CODEX_OUTPUT_USD_PER_1M: f64 = 10.00;
    const PER_1M: f64 = 1_000_000.0;

    (input_tokens as f64) * CODEX_INPUT_USD_PER_1M / PER_1M
        + (output_tokens as f64) * CODEX_OUTPUT_USD_PER_1M / PER_1M
}
