# Changelog

## [Unreleased]

### Added (round 22) — LM Studio tool calling: 3-layer test infrastructure

- `kangnam-harness-llm-bridge::test_util::MockLlmProvider` — scripted in-memory `LlmProviderDyn` for unit tests. State (steps queue + observed-call log) wrapped in `Arc<Mutex<…>>` so tests `clone()` once before boxing the original into `LlmAgent::new()`, then inspect `observed()` post-`run()`. `Step::text` / `Step::tool_call` / `Step::tool_calls` constructors. Gated behind the `test-util` feature.
- **Layer 1** `tests/loop_with_mock.rs` — 7 unit tests on the bridge loop with `MockLlmProvider`: terminal-text early return, single-tool-call dispatch + history shape (verifies `ChatContent::ToolUse` lands in the assistant turn), parallel calls dispatch in order, unknown-tool error returns the registered list, failed-tool propagates `is_error: true` to history, max-iterations caps runaway loops, and an explicit gate on the second request re-emitting `tool_calls` (the wire-correctness signal Round 20 unblocks).
- **Layer 2** `tests/lm_studio_wire.rs` — 2 wiremock-based integration tests against a fake OpenAI-compat HTTP server. Uses real `OpenAICompatProvider` from `kangnam-router` so the full request/response wire path is exercised. Round-trip: 1st call returns canned `tool_calls`; 2nd call (gated by `body_string_contains("tool_call_id")`) returns final text. Verifies the same network shape LM Studio at `http://localhost:1234/v1` would receive. Plus a failed-tool variant that asserts `Error: <msg>` lands in the `{role: "tool", tool_call_id, content}` follow-up.
- **Layer 3** `tests/lm_studio_live.rs` — `#[ignore]`-gated live LM Studio test. Reads `LMSTUDIO_BASE_URL` (e.g. `http://localhost:1234/v1`) + optional `LMSTUDIO_MODEL`. Registers a `multiply` tool, asks "What's 7 times 6?", asserts the model invoked `multiply(7, 6)` (commutative) and the final answer mentions 42. Skips early when env var is unset. Run with `LMSTUDIO_BASE_URL=http://localhost:1234/v1 cargo test -p kangnam-harness-llm-bridge --features test-util --test lm_studio_live -- --ignored`.
- `crates/harness/llm-bridge/Cargo.toml` — `[[test]]` declarations with `required-features = ["test-util"]` for `loop_with_mock` and `lm_studio_wire` so `cargo test --workspace` (which doesn't auto-enable features) skips them rather than failing compilation. `lm_studio_live` doesn't depend on `test-util` and is `#[ignore]`-gated.
- 13 bridge tests passing (3 unit + 7 layer-1 + 2 layer-2 + 1 doctest), 1 ignored layer-3 live test.

### Added (round 21) — `kangnam-harness-llm-bridge` crate

- New crate `crates/harness/llm-bridge/` bridges the harness `AgentTool` runtime to the router's multi-provider LLM clients with first-class tool calling.
- `LlmAgent<C = DefaultCapabilities>` builder: `new(provider, ctx)` → `with_tool(tool, description)` → `with_system_prompt` → `with_max_iterations` → `with_options` → `run(user_input)`. Generic over the harness capability bundle so domain-specific capability sets (Travel Planner, finance, …) instantiate `LlmAgent<TheirCaps>`.
- Multi-turn dispatch loop in `LlmAgent::run`: `chat_with_options_dyn` → if no `tool_calls`, return final text; otherwise record assistant turn via `ChatMessage::assistant_with_tool_calls` (Round 20 wire reconstruction), dispatch each call to the matching `AgentTool::execute`, push `ChatMessage::tool_result` for each result (with `is_error: true` for `ToolResult::Failed`), repeat up to `max_iterations` (default 8).
- `BridgeError` (Llm/UnknownTool/SuspendedTurn/MaxIterations). `SuspendedTurn` is a hard error: tools that call `ToolResult::AwaitUser` (interactive forms/previews) are incompatible with the autonomous loop and surface this so the host can drive the loop manually if needed.
- `ToolInvocation` / `AgentRun` records every dispatch. `AgentRun::messages` is the full conversation history including reconstructed assistant tool-call turns; `final_text` is the model's terminal answer; `tool_invocations` lists every dispatched call in order; `iterations` counts model round-trips.
- Workspace wiring: `kangnam-harness-llm-bridge` added to `[workspace.dependencies]`. `LM Studio` (OpenAI-compat) is the canonical target — `create_provider("openai_compat", "", "<model>", "http://localhost:1234/v1")` plus `with_tool(...)` is the standard one-liner.

### Added (round 20) — router `ChatContent::ToolUse` + assistant `tool_calls` wire reconstruction

- `kangnam_router::ChatContent::ToolUse { id, name, arguments }` — new `non_exhaustive` content-block variant lets multi-turn tool-calling loops record the assistant turn that emitted a tool call so the next request body re-pairs `tool_call_id` against the originating call.
- `kangnam_router::ChatMessage::assistant_with_tool_calls(text, calls)` constructor folds optional pre-call narration plus N `ToolCall`s into a single assistant message.
- `openai_compat::build_messages` now emits a `tool_calls` array on assistant messages (with **stringified** `arguments` per the OpenAI/LM Studio spec) and `content: null` when the assistant turn has only tool calls and no text. Matches LM Studio, vLLM, llama.cpp-server, and Ollama wire expectations.
- `claude.rs`: added `ContentBlock::ToolUse { id, name, input }` variant + match arm. Emits Anthropic-native `tool_use` blocks. `cache_control` for `ContentBlock::ToolUse` is rejected with a `tracing::warn!` matching the Thinking-block treatment.
- `gemini.rs`: added `GeminiPart::FunctionCall { function_call: GeminiFunctionCallOut }` variant + match arm. Emits Gemini-native `functionCall` parts on the model turn (paired with the existing `functionResponse` for `ToolResult`).
- `codex.rs` (OpenAI Responses API): emits a top-level `function_call` input item with stringified `arguments` and `call_id` matching the existing `function_call_output` convention used for `ToolResult`.
- `copilot.rs`: same OpenAI Chat Completions wire shape as `openai_compat` — `tool_calls` array on assistant messages with stringified arguments.
- `claude_local.rs` / `codex_local.rs` / `gemini_local.rs`: skip with `tracing::debug!` (CLI providers handle their own internal tool sets and don't surface user-defined tool calls via this API).
- Regression lock test `assistant_tool_calls_arguments_are_stringified_round_trip` in `openai_compat` verifies the assistant `tool_calls` array AND its stringified `arguments` field both land on the wire correctly. Router tests: 181 → 182 (no regressions across 207 router tests).

### Added (round 14)
- `kangnam-design-skill::docs/` — vendored protocol references from open-design `docs/`: `skills-protocol.md` (355 lines, full `od:` namespace spec including `od.craft.requires`, `od.kangnam_design_system`, mode/platform/scenario taxonomy) and `modes.md` (273 lines, the 7 mode/surface combinations: prototype, deck, template, design-system, image, video, audio). Linked from the crate-level docs at `lib.rs`.

### Added (round 13)
- `kangnam-design-contracts::sidecar` — port of upstream `@open-design/packages/sidecar-proto` (boundary types only; the Node-specific spawn/IPC implementation in `@open-design/sidecar` is intentionally not ported). New types: `AppKey` (3 lowercase: daemon/desktop/web), `APP_KEYS` slice, `SidecarMode` (dev/runtime), `SidecarSource` (3 kebab-case: packaged/tools-dev/tools-pack), constants modules `env::*` (10 env-var names), `stamp_flags::*` (5 CLI flags), `defaults::*` (host/ipcBase/namespace/projectTmpDirName/windowsPipePrefix), `messages::*` (6 message-tag constants), `SidecarErrorCode` (2 SCREAMING_SNAKE), `SidecarContractError`, `SidecarStamp` with `validate()` enforcing the upstream `normalizeIpcPath`/`normalizeNamespace` constraints, `SidecarStampCriteria`, `ServiceRuntimeState` (5), `DesktopRuntimeState` (3), `DaemonStatusSnapshot` / `WebStatusSnapshot` (alias) / `DesktopStatusSnapshot` (camelCase, double-Option for nullable pid/title), desktop intro shapes (`DesktopEvalInput`/`DesktopEvalResult`, screenshot/console/click), `ShutdownAccepted` locked-true newtype + `ShutdownResult`, `SharedSidecarMessage` (status/shutdown enum, used for daemon + web sidecars via aliases), `DesktopSidecarMessage` (5 variants: shared 2 + console + eval/screenshot/click with payloads), `normalize_namespace`/`normalize_ipc_path`/`is_windows_named_pipe_path` validators. 23 unit tests; 173 total in design-contracts (was 150).

### Added (round 12)
- `kangnam-design-contracts::critique` — port of upstream `critique.ts` (zod-heavy debate config + panel-event protocol). New types: `CRITIQUE_PROTOCOL_VERSION`, `PanelistRole` (5 lowercase), `PANELIST_ROLES` slice, `FallbackPolicy` (3 snake_case), `FALLBACK_POLICIES` slice, `RoleWeights` (5 floats with `Default` matching upstream `defaultCritiqueConfig().weights`), `CritiqueConfig` with explicit `validate() -> Result<(), CritiqueConfigError>` enforcing the upstream zod refinements (range checks, weights `[0,1]`, cross-field `scoreThreshold ≤ scoreScale + ε`), `CritiqueConfig::defaults()` mirroring `defaultCritiqueConfig()`, `CritiqueConfigError` (3 variants: `EmptyCast`, `OutOfRange`, `ThresholdExceedsScale`). Panel events: `DegradedReason` (5), `FailedCause` (4), `ParserWarningKind` (5), `RoundDecision` (continue/ship), `ShipStatus` (4 snake_case), `PanelArtifactRef`, `PanelEvent` 11-variant tagged enum (RunStarted / PanelistOpen / PanelistDim / PanelistMustFix / PanelistClose / RoundEnd / Ship / Degraded / Interrupted / Failed / ParserWarning) with snake_case discriminator + per-variant camelCase fields, `PanelEvent::sse_event_name()` and `PanelEvent::run_id()` accessors, `CRITIQUE_SSE_EVENT_NAMES` slice (11 wire event names), `is_panel_event(value)` predicate. **All upstream contracts modules now ported except `examples.ts` (fixtures) and `prompts/system.ts` (covered separately by `kangnam-design-prompt`).** 20 unit tests; 150 total in design-contracts (was 130).

### Added (round 11)
- `kangnam-design-contracts::api::projects` — final api/* module ported. New types: `ProjectKind` (7 lowercase), `MediaAspect` (5 — `1:1`/`16:9`/`9:16`/`4:3`/`3:4`), `AudioKind` (3 — music/speech/sfx), `ProjectDisplayStatus` (7 snake_case), `ProjectStatusInfo`, `PromptTemplateMetadataSource`, `PromptTemplateMetadataSurface` (image/video), `PromptTemplateMetadata`, `ProjectFidelity` (kebab), `ProjectIntent` (locked `live-artifact`), `ProjectMetadata` (24 optional fields), `Project`, `ProjectTemplate`/`ProjectTemplateFile`, `Conversation`, `CreateProjectRequest` / `UpdateProjectRequest` (with double-Option for nullable fields), `ProjectsResponse` / `ProjectResponse` / `CreateProjectResponse` (flatten + conversationId), `ConversationsResponse` / `ConversationResponse`, `CreateConversationRequest` / `UpdateConversationRequest`, `MessagesResponse`. **Deployments**: `DeployProviderId` (locked `vercel-self`), `DeploymentStatus` (6 kebab), `DeployTarget` (locked `preview`), `DeployConfigResponse`, `UpdateDeployConfigRequest`, `DeploymentInfo`, `ProjectDeploymentsResponse`, `DeployProjectFileRequest`, `DeployProjectFileResponse` / `CheckDeploymentLinkResponse` (aliases). **Preflight**: `DeployPreflightWarningCode` (9 kebab), `DeployPreflightWarning`, `DeployPreflightFile`, `DeployPreflightRequest`, `DeployPreflightResponse`. 16 unit tests. **All 11 upstream `api/*` files now ported** (130 tests in design-contracts, was 114).

### Added (round 10)
- `kangnam-design-contracts::api::connectors` — MCP-style connector port. New types: `ConnectorStatus` (4 lowercase), `ConnectorToolSideEffect` (4), `ConnectorToolApproval` (3), `ConnectorToolSafety`, `ConnectorToolDetail`, `ConnectorAuthProvider` (4 — local/none/oauth/composio), `ConnectorAuthDetail`, `ConnectorDetail`, `ConnectorListResponse`, `ConnectorStatusSummary` / `ConnectorStatusResponse`, `ConnectorDiscoveryProvider` (locked `composio`), `ConnectorDiscoveryMeta`, `ConnectorDiscoveryResponse`, `ConnectorDetailResponse`, `ConnectorConnectAuthKind` (3 snake), `ConnectorConnectAuth`, `ConnectorConnectResponse`, `ConnectorExecuteRequest`, `ConnectorExecuteOk` (locked `true` newtype), `ConnectorExecuteResponse`. 11 unit tests.
- `kangnam-design-contracts::api::registry` — daemon catalog projections. New types: `AgentModelOption`, `AgentInfo` (with double-Option for `version`), `AgentsResponse`, `SkillMode` (7 kebab-case), `SkillSurface` (4), `SkillPlatform` (2), `SkillFidelity` (2 kebab-case), `SkillSummary` (with double-Option for 5 nullable fields including `craftRequires`), `SkillDetail` (flatten + body), `SkillsResponse` / `SkillResponse`, `DesignSystemSummary` / `DesignSystemDetail`, `DesignSystemsResponse` / `DesignSystemResponse`, `HealthService` (locked `daemon`), `HealthOk` (locked `true`), `HealthResponse`, `CodexPetSummary` / `CodexPetsResponse`, `SyncCommunityPetsSource` (3), `SyncCommunityPetsRequest` / `SyncCommunityPetsResponse`. 10 unit tests; 114 total in design-contracts (was 93).

### Added (round 9)
- `kangnam-design-contracts::api::live_artifacts` — full live-artifact lifecycle port. New types: `BoundedJsonValue` (alias for `serde_json::Value`) + `BoundedJsonObject` (`HashMap<String, BoundedJsonValue>`), `LiveArtifactStatus` (3), `LiveArtifactRefreshStatus` (5), `LiveArtifactPreviewType` (3), `LiveArtifactSourceType` (3 snake_case), `LiveArtifactConnectorApprovalPolicy` (2 snake_case), `LiveArtifactRefreshPermission` (2), `LiveArtifactOutputTransform` (3 snake_case), `LiveArtifactProvenanceGenerator` (2), `LiveArtifactProvenanceSourceType` (4 snake_case), `LiveArtifactPreview`, `LiveArtifactDocumentFormat` (locked single-variant enum for `html_template_v1`), `LiveArtifactDocument`, `LiveArtifactSourceConnector`, `LiveArtifactOutputMappingPath` + `LiveArtifactOutputMapping`, `LiveArtifactSource`, `LiveArtifactProvenanceSource` + `LiveArtifactProvenance`, `LiveArtifactSchemaVersion` (locked-to-1 newtype), `LiveArtifact`, `DAEMON_OWNED_INPUT_FIELDS` (8 entries), `LiveArtifactCreateInput` / `LiveArtifactUpdateInput` (omit daemon-owned fields), `LiveArtifactSummary` (replaces `document` with `hasDocument: bool`), `LiveArtifactListResponse` / `LiveArtifactDetailResponse`, `LiveArtifactRefreshTerminalStatus` (locked single-variant `succeeded`), `LiveArtifactRefreshSummary` / `LiveArtifactRefreshResponse`, `LiveArtifactRefreshStepStatus` (5), `LiveArtifactRefreshErrorRecord`, `LiveArtifactRefreshSourceTypeTag` (locked `document`), `LiveArtifactRefreshSourceMetadata`, `LiveArtifactRefreshLogEntry` / `LiveArtifactRefreshLogResponse`. `BoundedJsonObject` / `BoundedJsonValue` re-exported from `kangnam_design_contracts::api` for use by `connectors` (planned R10) and other dependents. 11 new tests; 93 total in design-contracts.

### Added (round 8)
- `kangnam-design-contracts::api::comments` — `PreviewCommentStatus` (6 snake_case variants), `PreviewCommentPosition` (f64 box), `PreviewCommentSelectionKind`, `PreviewCommentMember`, `PreviewCommentTarget`, `PreviewComment`, `PreviewCommentUpsertRequest`, `PreviewCommentStatusRequest`, response envelopes, `PreviewCommentDeleteResponse` alias.
- `kangnam-design-contracts::api::chat` — full chat-run + persisted-event port: `ChatRole`, `ChatRunStatus` (5 lowercase, `canceled` single-l per upstream), `ChatRequest` / `ChatRunCreateRequest` (using double-Option for the 6 `string | null | undefined` fields), `ChatRunCreateResponse`, `ChatRunStatusResponse`, `ChatRunListResponse`, `ChatRunCancelResponse`, `ChatAttachment`, `ChatAttachmentKind`, `ChatCommentAttachment`, `ChatCommentAttachmentSource` (kebab-case), `PersistedAgentEvent` (9-variant tagged enum mirroring SSE `DaemonAgentPayload` but persisted-form: drops `thinking_start` marker, flattens token usage onto the variant, snake_case `kind` discriminator with camelCase per-variant fields), `PersistedLiveArtifactAction`, `PersistedLiveArtifactRefreshPhase`, `ChatMessage`. 82 passing tests in design-contracts (was 63).

### Added (round 7)
- `kangnam-design-contracts::api`: 5 new REST module ports — `app_config` (`AppConfigPrefs` with double-Option for `agentId`/`skillId`/`designSystemId`, `AgentModelPrefs`, `AppConfigResponse`, `UpdateAppConfigRequest`), `version` (`AppVersionInfo`, `AppVersionResponse`), `proxy` (`ProxyMessage`, `ProxyMessageRole`, `ProxyStreamRequest`, `ProxyStreamStartPayload`, `ProxyStreamDeltaPayload`, `ProxyStreamEndPayload`), `artifacts` (`ArtifactKind`, `ArtifactRendererId`, `ArtifactExportKind`, `ArtifactStatus`, `ArtifactManifest` with version-locked `ManifestVersion`, `SaveArtifactRequest`/`SaveArtifactResponse`), `files` (`ProjectFile`, `ProjectFileKind`, `ProjectFileType`, `ProjectFilesResponse`/`ProjectFileResponse`, `UploadProjectFilesResponse`, `DeleteProjectFileResponse`).
- `kangnam-design-contracts::serde_helpers::double_option` — adapter module for `T | null | undefined` shapes that distinguish "explicitly cleared" (`null`) from "not provided" (omitted). Three states map to `Option<Option<T>>`. 3 unit tests.
- `kangnam-design-contracts::sse::ProxySseEvent` upgraded from `serde_json::Value` placeholders to typed payloads (`ProxyStreamStartPayload`, `ProxyStreamDeltaPayload`, `ProxyStreamEndPayload`) now that `api::proxy` is ported.

### Changed
- **Workspace deps hoisting**: 5 deps used by ≥3 design crates (`async-stream`, `futures`, `tera`, `tracing`, `serde_yaml`) hoisted to `[workspace.dependencies]`. Per-crate Cargo.toml lines change from hardcoded version strings (`tera = "1"`) to `{ workspace = true }`; future design crates pick them up free.
- **Clippy clean**: full design family (18 crates) is now clippy-clean on the default lint set. Fixes include `manual_split_once` (design-skill frontmatter), `doc_lazy_continuation` (design-contracts api/files), `useless_vec` (design-prompt-template tests), `io_other_error` + `get_first` (design-tools), `should_implement_trait` rename `AiProvider::from_str` → `from_slug` (design-llm), `wrong_self_convention` `to_*` methods receiving `self` instead of `&self` for `Copy` types (design-doc-site `ManifestColor::to_css`, design-export-pptx `OuterShadow::to_ooxml_effect_xml`), `unnecessary_cast` (design-export-pptx color.rs), `manual_strip` (design-export-pptx color_convert.rs), `redundant_closure` (design-export-pptx writer/notes.rs), `let_and_return` (design-export-pptx test), `field_reassign_with_default` allow on a non-exhaustive struct, `single_match_else` (design-artifact parser test).
- `kangnam-design-contracts`: extracted repeated locked-value boilerplate into two reusable macros — `locked_true!` (replaces 4 hand-rolled bare-bool newtypes: `ConnectorExecuteOk`, `HealthOk`, `ShutdownAccepted`, plus the macro itself enables future locked-true sites) and `locked_u32!` (replaces 2 hand-rolled schema-version newtypes: `ManifestVersion`, `LiveArtifactSchemaVersion`). Net: 135 lines of boilerplate → 110 lines of single-source macro definition + 6 macro invocations of ~5 lines each. New locked-value newtypes can now be declared with one macro call instead of ~30 lines of `Serialize`/`Deserialize` impls. 175 unit tests passing (was 173); 2 new doctests.
- `kangnam-design` umbrella now re-exports the catalog/spec sister crates (`skill`, `system`, `direction`, `prompt`, `artifact`) under matching feature flags (all in default set). The umbrella's `craft` feature additionally propagates to `kangnam-design-skill`'s new `craft` feature so `DesignSkill::resolve_crafts()` is wired up automatically when the umbrella's `craft` feature is on.
- `kangnam-design-skill`: new optional `craft` feature gates a `DesignSkill::resolve_crafts() -> Vec<&'static Craft>` bridge to `kangnam-design-craft`. Without the feature the crate stays craft-agnostic (no extra dep). Replaces the previous "caller must call `kangnam_design_craft::requires_to_crafts(&skill.od.craft.requires)` themselves" ergonomics with a one-method shortcut.
- `kangnam-design-prompt-template`: new `TemplateFilter` extension trait — adds chainable `with_surface` / `with_tag` / `with_category` / `with_model` filters on `Iterator<Item = &PromptTemplate>`, with named iterator types (`WithSurface`, `WithTag`, `WithCategory`, `WithModel`) so the filter pipelines stay zero-alloc.

### Added
- **New crate `kangnam-design-html-template`** — vendored HTML scaffold templates for deck-mode skills (`DECK_FRAMEWORK`, `KAMI_DECK`). Embedded as `&'static str` via `include_str!`. Public API: `HtmlTemplate` struct (id, title, when_to_use, body), `TEMPLATES` slice, `template_by_id(id)` lookup. 5 unit tests + 1 doctest. Re-exported from `kangnam-design` umbrella under the new `html-template` feature flag (in default set). Counterpart to `kangnam-design-prompt` (system prompts) and `kangnam-design-prompt-template` (gen-media prompts).
- **New crate `kangnam-design-contracts`** — pure-Rust port of open-design's `@open-design/contracts` package. Wire-compatible with the upstream TypeScript zod schemas (camelCase serialization, lowercase enum variants, optional fields skip-on-`None`). Modules: `common` (`OkResponse`, `IdResponse`, `BoundedJsonConstraints`, `LIVE_ARTIFACT_BOUNDED_JSON_CONSTRAINTS`), `errors` (42-variant `ApiErrorCode` enum with strict deserialization, `ApiError` envelope, `ApiValidationIssue`, `SseErrorPayload`, `create_api_error`/`create_api_error_response` helpers), `tasks` (6-state `TaskState` enum with `is_terminal()` guard, `TaskStatus` snapshot, `TASK_STATES` slice), `sse` (generic `SseEvent<P>` envelope, discriminated `DaemonAgentPayload` covering text-delta / thinking / tool-use / tool-result / live-artifact / usage / raw, `ChatSseEvent` + `ProxySseEvent` event-name discriminators, `CHAT_SSE_PROTOCOL_VERSION` / `PROXY_SSE_PROTOCOL_VERSION`). 34 unit tests. Re-exported from `kangnam-design` umbrella under the new `contracts` feature flag (in default set). API endpoints (`api/*`), `critique.ts`, and `examples.ts` remain TODO.
- **New crate `kangnam-design-prompt-template`** — catalog of ready-made image / video generation prompt templates. Vendors **94 templates** from open-design v0.4 (44 image: profile-avatar, social-media, illustration, infographic, e-commerce, …; 50 video: cinematic, hyperframes, seedance, K-pop dance, retro, …) with their original schema (`id`, `surface`, `title`, `summary`, `category`, `tags`, `model`, `aspect`, `prompt`, `previewImageUrl`, `previewVideoUrl`, `source: {repo, license, author, url}`). Public API: typed `PromptTemplate` + `Surface` enum (`#[non_exhaustive]`), `load_templates_from_dir` / `load_templates_from_surface_dir` / `list_template_ids`, `has_tag` / `in_category` filter helpers, JSON-path-aware error reporting, round-trip preservation of unknown fields. 12 unit tests + 1 doctest. Distinct from `kangnam-design-prompt` (system-prompt composer). Re-exported from `kangnam-design` umbrella under the new `prompt-template` feature flag (in default set).
- **New crate `kangnam-design-craft`** — brand-agnostic craft references. Vendors the open-design v0.4 `craft/` directory (typography, color, anti-AI-slop, accessibility-baseline, animation-discipline, rtl-and-bidi, state-coverage) as `&'static str` constants via `include_str!`, plus a runtime loader for user-supplied craft files. Public API:
  - `Craft` (zero-alloc static record) + `OwnedCraft` (heap-loaded variant).
  - 7 built-in constants (`TYPOGRAPHY`, `COLOR`, `ANTI_AI_SLOP`, `STATE_COVERAGE`, `ANIMATION_DISCIPLINE`, `ACCESSIBILITY_BASELINE`, `RTL_AND_BIDI`) + `BUILTIN_CRAFTS` slice.
  - `craft_by_id(id)` — single-slug lookup.
  - `requires_to_crafts(slugs)` — resolve a `od.craft.requires` list, preserving order, dedupe, drop unknowns silently (forward-compat).
  - `render_for_prompt(crafts)` — concatenate into one system-prompt block (`## <title>` per section).
  - `load_crafts_from_dir(path)` + `list_craft_ids(path)` — disk loader for project-vendored crafts.
  - `AsCraftRef` polymorphic adapter so `Craft`, `OwnedCraft`, and `CraftRef` mix in one render call.
  - 17 unit tests + 2 doctests; covers letter-spacing assertion against vendored typography body.
  - Adapted from MIT-licensed [refero_skill](https://github.com/referodesign/refero_skill) via open-design (Apache-2.0) — both attributions preserved in crate docs.
- `kangnam-design-skill::OdMetadata.craft: OdCraft { requires: Vec<String> }` — typed parser for skills' `od.craft.requires` block (was previously captured anonymously in the `extras` flatten map). Resolution via `kangnam_design_craft::requires_to_crafts(&skill.od.craft.requires)` is intentionally left to the caller — design-skill stays craft-agnostic to avoid cross-crate coupling.
- `kangnam-design`: new `craft` feature flag (in default set) — re-exports `kangnam-design-craft` as `design::craft`.
- `kangnam-design-skill`: vendored catalog grew from **30 → 64 skills** by absorbing the open-design v0.4 catalog. New: `audio-jingle`, `design-brief`, `hatch-pet`, the entire `html-ppt` family (16 themes — pitch-deck, course-module, weekly-report, taste-brutalist/editorial, xhs-pastel-card/post/white-editorial, hermes-cyber-terminal, graphify-dark-graph, knowledge-arch-blueprint, obsidian-claude-gradient, presenter-mode-reveal, product-launch, tech-sharing, testing-safety-alert, dir-key-nav-minimal), `hyperframes`, `image-poster`, `kami-deck`, `kami-landing`, `live-artifact`, `open-design-landing`, `open-design-landing-deck`, `pptx-html-fidelity-audit`, `replit-deck`, `video-shortform`, `web-prototype-taste-brutalist`, `web-prototype-taste-editorial`, `web-prototype-taste-soft`. Each skill retains its bundled LICENSE file where present (e.g. `html-ppt`, `hatch-pet`, `guizang-ppt`).
- `kangnam-design-system`: vendored catalog grew from **73 → 139 systems** by absorbing the open-design v0.4 catalog. New themed systems include `agentic`, `ant`, `arc`, `atelier-zero`, `bento`, `brutalism`, `canva`, `claymorphism`, `cosmic`, `discord`, `dithered`, `doodle`, `duolingo`, `editorial`, `enterprise`, `fantasy`, `flat`, `friendly`, `futuristic`, `github`, `glassmorphism`, `gradient`, `huggingface`, `kami`, `levels`, `lingo`, `luxury`, `material`, `minimal`, `modern`, `mono`, `neobrutalism`, `neon`, `neumorphism`, `openai`, `pacman`, `paper`, `perspective`, `premium`, `professional`, `publication`, `refined`, `retro`, `shadcn`, `simple`, `skeumorphism`, `sleek`, `spacious`, `storytelling`, `tetris`, `vibrant`, `vintage`, plus modifier systems (`application`, `dashboard`, etc.).

### Changed
- Loader test floors raised: skills `>= 60` (was 25), systems `>= 130` (was 60). Canonical-id sanity checks extended (`html-ppt`, `hatch-pet`, `kami-deck` for skills; `agentic`, `shadcn`, `discord` for systems).

## [0.3.5] — 2026-04-28

### Fixed
- **Bug fix (visual change)**: `Background::Gradient.angle_deg` and `Fill::LinearGradient.angle_deg` now use the **same CSS convention** (0° = up, clockwise). Previously `Background::Gradient` was multiplied raw by 60_000 — meaning a SlideDoc passing `angle_deg: 90` (CSS = "to right") rendered as OOXML 90° = "down" in the exported PPTX, silently rotating gradients 90°. The `from_slide_doc` bridge was already passing CSS-degree values into the writer, so this was a cross-crate correctness issue. Consumers who hand-crafted `PptxDeck` with `Background::Gradient` and were compensating for the bug will see a 90° rotation in their output — pass `(old_deg + 90) % 360` to recover the previous direction.
- **Bug fix (visual change)**: `template::xml_ops::prst_geom_xml` for `RoundedRect` now correctly emits `adj = (radius_emu × 100_000) / min(w,h)` instead of `(radius_emu / min) × 50_000`. The previous formula produced corner radii **half** the requested size; templates editing rounded rects via `add_element` will now show the actual radius the caller asked for. The write-only `writer/shape.rs` path was already correct; this fix brings the template path into agreement.
- **Bug fix**: `ShapeBox::shadow` is now correctly emitted by the write-only `write_deck_to_bytes` path (previously silently dropped). Both write paths now emit byte-identical `<a:effectLst>` via the shared `OuterShadow::to_ooxml_effect_xml` helper.

### Changed
- `OuterShadow` gains `to_ooxml_effect_xml()` (`pub(crate)`) — the canonical XML emission helper, shared by the write-only and template-edit paths. Replaces `template::xml_ops::outer_shdw_xml` (now removed; was `pub(super)` so this is not a public API change).
- `geometry::roundrect_adj(radius_emu, w_emu, h_emu) -> i64` — new public helper consolidating the OOXML `roundRect` adj formula. Both `writer/shape.rs::emit_geometry` and `template/xml_ops.rs::prst_geom_xml` route through it.
- `writer/shape.rs::emit_fill` body collapsed from ~40 lines into a 4-line delegation to `Fill::to_ooxml_fill_xml()` (with a `TilePattern → noFill` special case for the write-only path). Eliminates the only place where a `Fill` change had to be made twice.
- `gs_list_xml` now `debug_assert!`s ascending stop position order — matches the existing doc-comment claim on `GradientStop`. Release builds still skip this check.
- `template::PptxTemplate::add_element` doc comment refreshed: no longer claims `Shape` returns an error (it has been fully implemented since v0.3.4).
- `upsert_slide_text_sp` error messages for missing `<p:txBody>` / `</p:txBody>` now include the placeholder idx so callers can map a failure back to the offending shape.

## [0.3.4] — 2026-04-25

### Added
- `OuterShadow` — new public type for outer drop shadow effects, mapped to OOXML `<a:effectLst><a:outerShdw>`. Fields: `dx_px`, `dy_px`, `blur_px` (all CSS-style pixels at 96 DPI), `color`, `alpha: Option<u32>` (per-mille, 0..=100_000). Re-exported from crate root.
- `ShapeBox::shadow: Option<OuterShadow>` — new field. `None` emits no `<a:effectLst>`. Shadow direction math: `dist = √(dx²+dy²) × 9525`, `dir = atan2(dy,dx) deg mod 360 × 60000` (matches `build_outer_shadow_xml` in dear-jeongbin `export_pptx_ooxml.rs`).
- `ShapeBox::new(frame, shape, fill, stroke) -> Self` — constructor that sets `shadow: None`. Required for external-crate construction now that `ShapeBox` is `#[non_exhaustive]`.
- `PptxTemplate::add_element(PptxElement::Shape(ShapeBox))` — fully implemented (previously returned `Err(Xml("scheduled for v0.3.4"))`). Handles all five fill variants: `Solid`, `LinearGradient`, `RadialGradient`, `TilePattern` (PNG embed + slide rels), and `None`. Also handles `Stroke` and `OuterShadow`. `Line` shape kind emits `prstGeom prst="line"` with forced `<a:noFill/>` (see TODO in code for potential `<p:cxnSp>` round-trip concern).
- 7 integration tests in `tests/template_shape.rs`: rect/rounded-rect/solid/gradient/tile/shadow + shadow direction math.

### Changed
- `#[non_exhaustive]` attribute added to `ShapeBox`. External-crate struct literals are now a compile error — use `ShapeBox::new(...)` + field mutation pattern.
- `PptxTemplate::add_tile_pattern_rect` marked `#[deprecated(since = "0.3.4")]`. Will be removed in v0.4.0. Use `add_element(PptxElement::Shape(ShapeBox { fill: Fill::TilePattern{..}, .. }))` instead.
- `Fill::to_ooxml_fill_xml` doc comment updated: no longer "transitional API scheduled for v0.3.5" — promoted to stable public API. Comments referencing "v0.3.5" corrected to "v0.3.4".

### Breaking changes
- `ShapeBox` has a new `shadow: Option<OuterShadow>` field. **Migration**: switch struct literals to `ShapeBox::new(frame, shape, fill, stroke)` (sets `shadow: None`), then assign `sb.shadow = Some(...)` if needed.
- `#[non_exhaustive]` on `ShapeBox` blocks external-crate struct literal construction (including `..Default::default()` spread). Use `ShapeBox::new`.
- `PptxTemplate::add_element(PptxElement::Shape(...))` no longer returns `Err` — it now succeeds. Callers that `unwrap_err()`'d this call will panic.

### Migration guide
```rust
// Before (any version — struct literal)
ShapeBox { frame, shape, fill, stroke: None }

// After (v0.3.4 — from any crate)
ShapeBox::new(frame, shape, fill, None)

// After (v0.3.4 — with shadow)
{
    let mut sb = ShapeBox::new(frame, shape, fill, None);
    sb.shadow = Some(OuterShadow { dx_px: 4.0, dy_px: 4.0, blur_px: 12.0, color: Color::BLACK, alpha: Some(40_000) });
    sb
}

// Before (add_tile_pattern_rect)
tmpl.add_tile_pattern_rect(slide, frame, &png_bytes, 0)?;

// After (v0.3.4 — add_element)
tmpl.add_element(slide, PptxElement::Shape(ShapeBox::new(frame, ShapeKind::Rect, Fill::TilePattern { png_bytes, tile_w_px: 24, tile_h_px: 24 }, None)))?;
```

## [0.3.3] — 2026-04-25

### Added
- `TextStyle` gains 3 new fields:
  - `italic: bool` (default `false`) → `<a:rPr i="1"/>` when `true`. Independent of `font_weight`.
  - `color_alpha: Option<u32>` (default `None`) — per-mille OOXML alpha (0..=100_000). `Some(50_000)` → `<a:srgbClr><a:alpha val="50000"/></a:srgbClr>`. `None` emits no `<a:alpha>` tag (fully opaque).
  - `allow_wrap: bool` (default `true`) → `<a:bodyPr wrap="square">` when `true`, `wrap="none"` when `false`.
- `#[non_exhaustive]` attribute added to `TextStyle` for forward compatibility with future field additions. External-crate consumers must use `Default::default()` + field mutation (struct literal syntax is no longer permitted outside the crate).
- `PptxTemplate::add_element(PptxElement::Text(TextBox))` is now fully implemented (previously returned `Err(Xml("scheduled for v0.3.3"))`). Emits a freeform `<p:sp txBox="1">` at absolute EMU coordinates with all `TextStyle` attrs applied.
- 9 integration tests in `tests/template_text.rs` covering all new attrs + alignment + newlines.

### Fixed
- **Bug fix (visual change)**: `TextStyle::line_height` was silently ignored by the v0.2 `write_deck` path (`writer/text.rs`). It is now correctly emitted as `<a:pPr><a:lnSpc><a:spcPct val="N"/></a:lnSpc></a:pPr>`. Conversion: `(line_height * 100_000).round() as u32` (CSS ratio → OOXML 1/1000-percent). Default `line_height = 1.2` → `val="120000"`. Consumers that relied on `line_height` being silently ignored will see a visual change in output PPTX — this is the intended behavior.

### Breaking changes
- `TextStyle` has 3 new required fields. **Migration**: add `..Default::default()` spread when constructing via struct literal (only valid within the same crate). External crates must use `Default::default()` + field mutation.
- `#[non_exhaustive]` on `TextStyle` means external-crate struct literals are now a compile error regardless of spread. Use `let mut s = TextStyle::default(); s.field = value; s` pattern.
- `PptxTemplate::add_element(PptxElement::Text(...))` no longer returns `Err` — it now succeeds. Callers that `unwrap_err()`'d this call will panic.

### Migration guide
```rust
// Before (v0.3.2, within same crate only)
TextStyle { font_family: "Pretendard".into(), font_size_pt: 24.0, ..Default::default() }

// After (v0.3.3, from any crate — including this crate's own tests)
{
    let mut s = TextStyle::default();
    s.font_family = "Pretendard".into();
    s.font_size_pt = 24.0;
    s
}
```

## [0.3.2] — 2026-04-25

### Added
- `canvas-pptx-writer`: `Fill` enum extended with three new variants:
  - `Fill::LinearGradient { angle_deg, stops: Vec<GradientStop> }` — multi-stop linear gradient. `angle_deg` follows CSS convention (0° = up, clockwise); converted to OOXML 1/60000-degree units internally.
  - `Fill::RadialGradient { stops: Vec<GradientStop> }` — multi-stop radial gradient (`<a:path path="circle">` centered at 50%/50%).
  - `Fill::TilePattern { png_bytes, tile_w_px, tile_h_px }` — 1:1 PNG tile fill, top-left aligned, embedded as `ppt/media/imageN.png`.
- New public type `GradientStop { position: f32, color: Color, alpha: Option<u32> }`. Re-exported from crate root.
- `Fill::dot_tile(tile_w_px, tile_h_px, dot_radius_px, color_rgba) -> Result<Fill>` — builds a `TilePattern` with an anti-aliased centered dot. Uses 2× supersampling + 2×2 box downsample (reduces LibreOffice bilinear brightening).
- `Fill::solid(color) -> Fill` — convenience constructor for fully-opaque solid fill (preferred over struct literal to avoid specifying the new `alpha` field).
- `Fill::to_ooxml_fill_xml(&self) -> String` — transitional public method that emits the OOXML fill fragment. Returns empty string for `TilePattern` (which requires slide-context zip mutation). Will be superseded by `add_element(Shape)` in v0.3.5.
- `PptxTemplate::add_tile_pattern_rect(slide, frame, png_bytes, border_radius_emu)` — transitional method that embeds a PNG tile and emits a `<p:sp>` with `<a:blipFill><a:tile/>` into the slide. Handles media embed, `[Content_Types].xml`, and slide rels automatically.
- `#[non_exhaustive]` attribute on `Fill` enum (forward-compatible for future variants).
- `image = "0.25"` (png feature only) added as default dependency for `Fill::dot_tile`.
- 8 integration tests in `tests/template_fills.rs` + 5 unit tests in `color.rs`.

### Changed
- `Fill::Gradient { from, to, angle_deg }` marked `#[deprecated(since = "0.3.2")]`. Kept for v0.2/v0.3.0 consumers; will be removed in v0.4.0. New code should use `Fill::LinearGradient`.
- `writer/shape.rs` and `from_slide_doc.rs` updated to handle new `Fill` variants (`TilePattern` falls back to `<a:noFill/>` in the write-only PptxDeck path since it requires slide context).

### Breaking changes
- `Fill::Solid { color }` → `Fill::Solid { color, alpha: Option<u32> }`. Any code constructing `Fill::Solid { color: c }` as a struct literal will fail to compile. **Migration**: use `Fill::solid(c)` or `Fill::Solid { color: c, alpha: None }`.
- `#[non_exhaustive]` on `Fill` means exhaustive match patterns (`_ => ...` wildcard not required in v0.3.1) now require a wildcard arm if all existing variants were matched. This affects code that matched all v0.3.1 variants without a wildcard.

### Migration guide
```rust
// Before (v0.3.1)
let fill = Fill::Solid { color: my_color };

// After (v0.3.2) — option A (preferred)
let fill = Fill::solid(my_color);

// After (v0.3.2) — option B (explicit)
let fill = Fill::Solid { color: my_color, alpha: None };
```

## [0.3.1] — 2026-04-25

### Added
- `canvas-pptx-writer`: `PptxTemplate::embed_font(typeface, variant, ttf_bytes)` for embedding custom TTF/OTF fonts (e.g. Pretendard) directly into a PPTX package.
  - New public type: `FontVariant` enum (`Regular`, `Bold`, `Italic`, `BoldItalic`). Re-exported from the crate root.
  - Mutates 4 in-memory entries on each call: appends `ppt/fonts/fontN.fntdata`, adds `<Override>` in `[Content_Types].xml`, adds `<Relationship Type=".../font">` in `ppt/_rels/presentation.xml.rels`, and upserts `<p:embeddedFontLst>` in `ppt/presentation.xml`.
  - Multi-variant calls for the same typeface merge into one `<p:embeddedFont>` block (matching PowerPoint authoring behavior).
  - Pre-existing embedded fonts loaded from the template are counted at load time; newly embedded fonts get unique `fontN` IDs.
- 6 integration tests in `tests/template_font_embed.rs`.

### Note on font licensing
Embedding a TTF/OTF redistributes the font binary inside the resulting `.pptx`. This library does **not** validate font licenses; consumers are responsible for ensuring the typeface permits redistribution (Pretendard: SIL OFL ✅, Apple System Fonts: ❌).

## [0.3.0] — 2026-04-25

### Added
- `canvas-pptx-writer`: `PptxTemplate` template-editing path that loads an existing `.pptx` (with slideMaster, slideLayouts, theme, embedded fonts), appends slides inheriting from layouts, fills `<p:ph idx="N"/>` placeholders, and re-zips. Counterpart to the write-only `PptxDeck` IR.
  - New types: `PptxTemplate`, `SlideRef`.
  - New methods: `load`, `load_bytes`, `layout_count`, `slide_size_emu`, `add_slide_from_layout`, `set_placeholder_text`, `set_placeholder_image`, `add_element` (Image variant; Text/Shape stubbed for v0.3.2-v0.3.4), `add_full_bleed_image`, `pack`, `write`.
  - Round-trip preserves untouched zip entries byte-identical (theme/master/layout). Layout/placeholder lookup errors are typed.
- `canvas-pptx-writer`: `PptxWriteError` gains `InvalidTemplate`, `LayoutNotFound`, `PlaceholderNotFound`, `SlideNotFound` variants and `#[non_exhaustive]`.
- 14 integration tests covering full lifecycle: load → add_slide → set_placeholder_text → set_placeholder_image → add_full_bleed_image → pack.

## [0.2.0] — 2026-04-24

### Added
- Workspace split into five crates: `canvas-slide-doc`, `canvas-llm`, `canvas-editor`, `canvas-pptx-writer`, and the umbrella `canvas`.
- `canvas-pptx-writer` gains `slide-doc` feature (default-on) that exposes `from_deck` / `from_slide_doc` helpers.
- `canvas-llm` gains `test-util` feature exposing `FakeAiClient`.

### Changed
- `canvas-pptx-writer` bumps from 0.1.0 → 0.2.0 (still backward-compatible for write-only consumers who disable `slide-doc`).

### Migration
- Direct consumers of `canvas-pptx-writer` v0.1: no change required (the old API is still there; the adapter is additive and opt-in).
- New integrations: add `canvas = { version = "0.2", features = ["full"] }` and import from `canvas::*` / `canvas::editor::*` / `canvas::pptx::*`.

## [0.1.0] — 2026-04-24

Initial release (canvas-pptx-writer only).
