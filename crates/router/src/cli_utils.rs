//! Utilities for spawning CLI subprocesses, primarily used by the `_local`
//! providers but exposed publicly for downstream apps building their own
//! provider integrations.

use std::path::PathBuf;

// ─── Image temp-file helpers ────────────────────────────────────────────────

/// RAII guard that deletes a temporary file when dropped.
///
/// Created by [`decode_base64_image`] to hold decoded image bytes until the
/// CLI subprocess finishes. Dropping this value (or the `Vec<TempFile>` it
/// lives in) removes the file from disk.
pub struct TempFile(pub PathBuf);

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Map a MIME type string to a canonical file extension used for temp files.
///
/// Falls back to `"bin"` for unrecognised types so the CLI at least receives
/// *some* file (the provider may reject it, but that is a caller problem).
fn mime_to_ext(mime_type: &str) -> &'static str {
    match mime_type {
        "image/png" => "png",
        "image/jpeg" | "image/jpg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/bmp" => "bmp",
        _ => "bin",
    }
}

/// Decode a base64-encoded image, write it to a uniquely named temporary file,
/// and return a [`TempFile`] guard that deletes it on drop.
///
/// The file is placed in the OS temp directory with the name
/// `llm_router_img_<uuid>.<ext>` where `ext` is derived from `mime_type`.
///
/// # Errors
/// Returns `Err(String)` if base64 decoding fails or the temp file cannot be
/// written. The string carries a human-readable explanation.
pub fn decode_base64_image(data: &str, mime_type: &str) -> Result<TempFile, String> {
    use base64::Engine as _;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|e| format!("base64 decode failed: {e}"))?;

    let ext = mime_to_ext(mime_type);
    let filename = format!("llm_router_img_{}.{}", uuid::Uuid::new_v4(), ext);
    let path = std::env::temp_dir().join(filename);

    std::fs::write(&path, &bytes).map_err(|e| format!("write temp image failed: {e}"))?;

    Ok(TempFile(path))
}

/// Removes characters that break execve / CLI argument passing:
/// - Null bytes (`\0`): execve forbids these in argv
/// - ASCII C0 controls (U+0000–U+001F) except `\t`, `\n`, `\r`
/// - DEL (U+007F) and C1 controls (U+0080–U+009F): stripped by
///   `char::is_control()` — common junk in Latin-1-decoded text.
///
/// Common need when prompts contain text extracted from PDF/DOCX/PPTX.
pub fn sanitize_prompt(input: &str) -> String {
    input
        .chars()
        .filter(|&c| c != '\0')
        .filter(|&c| c == '\t' || c == '\n' || c == '\r' || !c.is_control())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_null_bytes() {
        assert_eq!(sanitize_prompt("a\0b\0c"), "abc");
    }

    #[test]
    fn removes_c0_controls_except_whitespace() {
        let input = "hello\x01\x07world\t\nfoo";
        assert_eq!(sanitize_prompt(input), "helloworld\t\nfoo");
    }

    #[test]
    fn preserves_normal_text() {
        let input = "한글 ABC 123 !@#$%";
        assert_eq!(sanitize_prompt(input), input);
    }

    #[test]
    fn preserves_tab_newline_carriage_return() {
        assert_eq!(sanitize_prompt("a\tb\nc\rd"), "a\tb\nc\rd");
    }

    #[test]
    fn empty_input() {
        assert_eq!(sanitize_prompt(""), "");
    }

    #[test]
    fn removes_del_and_c1_controls() {
        let input = "a\u{007F}b\u{0085}c\u{009F}d";
        assert_eq!(sanitize_prompt(input), "abcd");
    }
}

/// Resolves a CLI binary by name. Search order:
/// 1. Environment variable `${NAME_UPPER}_CLI_PATH` (e.g. `CLAUDE_CLI_PATH`)
/// 2. Common install dirs: /opt/homebrew/bin, /usr/local/bin, /usr/bin,
///    $HOME/.local/bin, $HOME/bin
/// 3. Shell `command -v <name>` (uses $SHELL or /bin/zsh)
/// 4. Bare name (relies on subprocess inheriting PATH)
///
/// Critical for desktop apps (Tauri/Electron) where the GUI launch context
/// has no shell PATH.
///
/// Names containing characters outside `[A-Za-z0-9._-]` skip the shell
/// lookup step (defense against shell injection) and fall through to the
/// bare-name fallback.
pub fn resolve_binary(name: &str) -> PathBuf {
    let env_key = format!("{}_CLI_PATH", name.to_ascii_uppercase());
    if let Ok(path) = std::env::var(&env_key) {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }

    for candidate in common_paths(name) {
        if candidate.exists() {
            return candidate;
        }
    }

    // Defensive: skip the shell step if `name` contains any character that
    // could break out of `command -v <name>` into arbitrary shell commands.
    // Fall through to the bare-name fallback instead.
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
    {
        return PathBuf::from(name);
    }

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
    let output = std::process::Command::new(shell)
        .arg("-lc")
        .arg(format!("command -v {name}"))
        .output();

    if let Ok(output) = output {
        if output.status.success() {
            let resolved = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !resolved.is_empty() {
                return PathBuf::from(resolved);
            }
        }
    }

    PathBuf::from(name)
}

fn common_paths(name: &str) -> Vec<PathBuf> {
    let mut candidates = vec![
        PathBuf::from("/opt/homebrew/bin").join(name),
        PathBuf::from("/usr/local/bin").join(name),
        PathBuf::from("/usr/bin").join(name),
    ];
    if let Ok(home) = std::env::var("HOME") {
        candidates.push(PathBuf::from(&home).join(".local/bin").join(name));
        candidates.push(PathBuf::from(home).join("bin").join(name));
    }
    candidates
}

/// Builds a PATH env string with common tool dirs prepended to current $PATH.
/// Use as `cmd.env("PATH", build_path_env())` before spawn — this is the only
/// reliable way to find `claude`/`codex`/`gemini` from a Tauri GUI launch.
pub fn build_path_env() -> String {
    let mut segments = Vec::new();

    for fixed in ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"] {
        push_unique(&mut segments, fixed.to_string());
    }
    if let Ok(home) = std::env::var("HOME") {
        push_unique(&mut segments, format!("{home}/.local/bin"));
        push_unique(&mut segments, format!("{home}/bin"));
    }
    if let Ok(existing) = std::env::var("PATH") {
        for segment in existing.split(':') {
            push_unique(&mut segments, segment.to_string());
        }
    }

    segments.join(":")
}

fn push_unique(segments: &mut Vec<String>, candidate: String) {
    let trimmed = candidate.trim();
    if trimmed.is_empty() {
        return;
    }
    if segments.iter().any(|s| s == trimmed) {
        return;
    }
    segments.push(trimmed.to_string());
}

#[cfg(test)]
mod resolve_tests {
    use super::*;

    #[allow(unsafe_code)]
    #[test]
    fn env_var_override_takes_priority() {
        // SAFETY: The env var NONEXISTENT_BINARY_XYZ_CLI_PATH is uniquely named
        // for this test and is never read by any other test in this binary.
        // cargo test is multi-threaded by default; std::env::{set,remove}_var
        // are unsound in the strict sense when other threads read env vars, but
        // no other test reads this specific key, so no race is possible.
        unsafe {
            std::env::set_var("NONEXISTENT_BINARY_XYZ_CLI_PATH", "/custom/path/to/binary");
        }
        let resolved = resolve_binary("nonexistent_binary_xyz");
        assert_eq!(resolved, PathBuf::from("/custom/path/to/binary"));
        // SAFETY: same as above — unique key, no concurrent readers.
        unsafe {
            std::env::remove_var("NONEXISTENT_BINARY_XYZ_CLI_PATH");
        }
    }

    #[test]
    fn returns_bare_name_for_unknown_binary() {
        let resolved = resolve_binary("definitely_does_not_exist_zzz_99");
        assert_eq!(resolved, PathBuf::from("definitely_does_not_exist_zzz_99"));
    }

    #[test]
    fn build_path_env_includes_common_dirs() {
        let path = build_path_env();
        assert!(path.contains("/opt/homebrew/bin"));
        assert!(path.contains("/usr/local/bin"));
        assert!(path.contains("/usr/bin"));
    }

    #[test]
    fn build_path_env_no_duplicates() {
        let path = build_path_env();
        let segments: Vec<&str> = path.split(':').collect();
        let mut seen = std::collections::HashSet::new();
        for segment in &segments {
            assert!(seen.insert(*segment), "duplicate segment: {segment}");
        }
    }

    #[test]
    fn names_with_shell_metacharacters_do_not_invoke_shell() {
        // This should return the bare name without spawning a shell, even though
        // the name isn't found on the filesystem. The test is mostly about the
        // shell-injection guard: if the guard is removed, the shell invocation
        // might crash or hang on malformed input.
        let resolved = resolve_binary("foo; rm -rf /");
        assert_eq!(resolved, PathBuf::from("foo; rm -rf /"));
    }
}
