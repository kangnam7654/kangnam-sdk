fn main() {
    tauri_build::build();

    // Rebuild if OAuth secrets change
    println!("cargo:rerun-if-env-changed=GEMINI_CLIENT_SECRET");
    println!("cargo:rerun-if-env-changed=ANTIGRAVITY_CLIENT_SECRET");
}
