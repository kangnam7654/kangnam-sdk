# kangnam-sdk crate migration

## Purpose

Move remaining standalone reusable Rust crates into `kangnam-sdk` and redirect local consumers away from the old per-crate Git repositories. Done means `kangnam-sdk` owns the source for the migrated crates, consumers refer to packages from `https://github.com/kangnam7654/kangnam-sdk`, and focused Cargo checks pass locally.

## File changes

| Path | Change |
|---|---|
| `/Users/kangnam/projects/kangnam-sdk/Cargo.toml` | Add `crates/fortune/*` workspace members and workspace dependencies for `saju-engine` and `tarot-engine`; add missing shared dependency versions used by migrated crates. |
| `/Users/kangnam/projects/kangnam-sdk/crates/fortune/saju-engine/**` | Vendor the existing `saju-engine` library source, tests, examples, README, changelog, and license as a normal workspace member. |
| `/Users/kangnam/projects/kangnam-sdk/crates/fortune/tarot-engine/**` | Vendor the existing `tarot-engine` library source, tests, examples, README, changelog, and license as a normal workspace member. |
| `/Users/kangnam/projects/kangnam-sdk/crates/router/**` | Convert the existing gitlink/nested checkout into normal tracked workspace files so `kangnam-sdk` can be cloned without a separate `llm-router` repository. |
| `/Users/kangnam/projects/lunawave/backend/Cargo.toml` | Point `pii-crypto`, `saju-engine`, `tarot-engine`, `llm-router`, `chat-core`, and `chat-agent` at `kangnam-sdk`, using `package = ...` where package names changed. |
| `/Users/kangnam/projects/lunawave/Cargo.toml` | Update commented local patch guidance from old crate repos to the `kangnam-sdk` source. |
| `/Users/kangnam/projects/travel-planner/backend/Cargo.toml` | Point `llm-router` at `kangnam-sdk` package `kangnam-router`. |
| `/Users/kangnam/projects/dear-jeongbin/src-tauri/Cargo.toml` | Point `llm-router` at `kangnam-sdk` package `kangnam-router`. Leave legacy `canvas-sdk` path deps unchanged in this pass because the new `design-*` API is not a drop-in replacement. |
| `/Users/kangnam/projects/reviewers/crates/backend/Cargo.toml` | Point `llm-router` at `kangnam-sdk` package `kangnam-router`. |
| `/Users/kangnam/projects/chat-sdk/Cargo.toml` | Point legacy `chat-sdk`'s own `llm-router` dependency at `kangnam-sdk` for compatibility builds. |
| `Cargo.lock` files | Refresh where Cargo can resolve dependencies locally. |

## Implementation order

1. Copy `saju-engine` and `tarot-engine` into `/Users/kangnam/projects/kangnam-sdk/crates/fortune/`.
2. Rewrite the new engine crate manifests to inherit workspace package metadata and workspace dependency versions.
3. Convert `/Users/kangnam/projects/kangnam-sdk/crates/router` from gitlink to ordinary workspace files without deleting source content.
4. Update `kangnam-sdk/Cargo.toml` workspace membership and dependencies.
5. Update consumer manifests to reference `git = "https://github.com/kangnam7654/kangnam-sdk", branch = "main"`.
6. Run `cargo fmt`, then focused `cargo test`/`cargo check` commands for `kangnam-sdk` and touched consumers.

## Function/API signatures

No Rust function signatures are intentionally changed. The migration preserves these package-level import surfaces:

```toml
saju-engine = { git = "https://github.com/kangnam7654/kangnam-sdk", branch = "main" }
tarot-engine = { git = "https://github.com/kangnam7654/kangnam-sdk", branch = "main" }
pii-crypto = { git = "https://github.com/kangnam7654/kangnam-sdk", branch = "main" }
llm-router = { package = "kangnam-router", git = "https://github.com/kangnam7654/kangnam-sdk", branch = "main" }
chat-core = { package = "kangnam-chat-core", git = "https://github.com/kangnam7654/kangnam-sdk", branch = "main" }
chat-agent = { package = "kangnam-chat-agent", git = "https://github.com/kangnam7654/kangnam-sdk", branch = "main" }
```

Existing Rust `use llm_router::...`, `use chat_core::...`, `use saju_engine::...`, and `use tarot_engine::...` call sites continue to compile through dependency aliases.

## Constraints

- Do not delete standalone repositories such as `/Users/kangnam/projects/saju-engine`, `/Users/kangnam/projects/tarot-engine`, or `/Users/kangnam/projects/llm-router`; other consumers may still depend on their tags.
- Do not rewrite legacy `canvas-sdk` consumers to `design-*` in this pass because the module names and re-export layout changed.
- Preserve user-authored dirty changes in `kangnam-sdk`; edits must compose with the existing edition/workspace-dependency cleanup already in progress.
- Keep library crates panic-safe and retain `unsafe_code = "deny"`.
- Consumer `Cargo.lock` files cannot be refreshed against the Git remote until the `kangnam-sdk` changes are committed and pushed, because the remote `main` must contain the new packages first.

## Decisions

- Adopted: package source moves into `kangnam-sdk`, while external consumers use `package = ...` aliases when SDK package names differ from legacy crate names.
- Rejected: deleting or archiving old standalone crate repositories in this turn, because existing tagged dependencies outside the scanned workspace may still rely on them.

## Verification

- `cargo test -p saju-engine -p tarot-engine`
- `cargo test -p kangnam-router` (requires local port binding for wiremock tests)
- `cargo test -p kangnam-chat-core -p kangnam-chat-agent`
- `cargo test -p pii-crypto`
- `cargo check --workspace`
