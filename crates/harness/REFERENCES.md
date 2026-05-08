# Harness — External References

`crates/harness/*` 설계 시 참조할 외부 LLM agent 프레임워크 / 런타임. 코드를 직접 베끼는 건 라이선스/언어 차이 때문에 위험하지만, 설계 패턴과 abstraction 분리는 훔칠 가치가 있다.

---

## Pi — `earendil-works/pi`

| | |
|---|---|
| Repo | https://github.com/earendil-works/pi |
| License | MIT |
| Stack | TypeScript 96.9%, npm workspaces monorepo |
| Maturity | 45.9k stars, 3,971 commits, 213 releases (v0.73.1, May 2026) |
| Domain | General-purpose coding agent toolkit + Slack bot + vLLM pods |
| Added | 2026-05-08 |

### 한 줄 요약
범용 코딩 에이전트 툴킷. CLI(`coding-agent`) + agent 런타임 + 통합 LLM API + TUI/Web UI를 npm workspace로 묶음. 철학: real-world OSS coding session 공유로 학습 (toy benchmark 거부).

### Package ↔ 우리 crate 대응

| Pi (TS) | Kangnam-SDK (Rust) | 메모 |
|---|---|---|
| `packages/agent` | `crates/harness/runtime` | AgentTool 추상화 |
| `packages/ai` | `crates/router` + `crates/harness/llm-bridge` | 멀티 프로바이더 |
| `packages/coding-agent` | `apps/kangnam-client` (부분) | CLI/UI |
| `packages/tui` | — | 우리는 Tauri UI |
| `packages/web-ui` | — | 우리는 Tauri 기반 |

### 훔칠 만한 패턴 (검토 후보)

1. **Steering messages (실행 중 주입)** — Pi는 tool 실행 중 사용자 메시지를 큐에 넣어 다음 turn 직전 합류. 우리 `InteractionBridge`(suspend/resume)와 다른 모델 — 비교 검토 가치.
2. **Follow-up messages (post-completion 큐)** — agent가 완료된 직후 자동으로 다음 메시지 처리. 우리 runtime에 명시적 대응 없음.
3. **`AgentMessage` ↔ LLM message 분리** — Pi의 `convertToLlm`이 application-level 메시지를 LLM-compat 형태로 필터/변환. 우리 `llm-bridge`가 비슷한 역할이지만 명시적 분리는 약함.
4. **Tool `terminate: true` hint** — tool 결과가 자동 follow-up LLM 호출을 스킵하라고 신호. 비용 + 응답 빠름. `AgentTool::call()` 반환 타입에 추가 고려.
5. **`executionMode: parallel | sequential`** — tool 단위로 override 가능. 우리 runtime이 어떤 모델인지 확인 후 차이 검토.
6. **Cross-provider handoffs + thinking block transform** — 한 conversation을 provider 갈아타며 진행. Anthropic의 thinking 블록을 다른 provider로 변환. `kangnam-router`/`design-llm/models` 확장 후보.
7. **Auto model discovery + cost tracking** — provider별 모델 목록 자동 발견 + 토큰/비용 추적. 우리 `design-llm/src/models/` 비슷하지만 cost 추적은 미구현.
8. **Streaming partial tool arguments** — tool 인자가 stream되면서 partial 검증. UX 응답성 향상.

### 적용하지 말 패턴 / 우리와 안 맞는 부분

- **`streamProxy` (브라우저 → 백엔드 위임)** — 브라우저에서 API key 노출 회피용. 우리는 Tauri 네이티브라 키가 OS 키체인에 머물러 불필요.
- **TypeBox runtime schema 검증** — 우리는 `serde` 컴파일 타임 검증. Rust trait + thiserror 조합이 더 안전.
- **`pi-share-hf` (HuggingFace 세션 공유)** — 우리 라이선스는 독점이고 사용자 데이터 외부 공유는 정책 외.
- **`vLLM pods` 통합** — 우리는 LM Studio + 클라우드 provider만, 자체 모델 호스팅 안 함.

### 1차 시사점 (요약)

가장 매력적인 import 후보 3개:
1. **Tool `terminate` hint** — 1줄 변경에 가까운 ROI 큰 추가. `AgentTool` trait 반환에 `should_skip_followup: bool` 같은 필드.
2. **Steering / follow-up message 큐 모델** — 우리 `InteractionBridge`가 suspend/resume 모델인데, 큐 모델이 동시 사용자 입력에 더 자연. 비교 ADR 가치.
3. **Cost/token tracking** — `design-llm` 확장. provider별 가격표 + per-call 누적. 운영 시 즉시 가치.

### Reference 형식 — 다음 reference 추가 시

이 파일에 같은 표 + 섹션 구조로 append. 다른 framework 추가 후보:
- LangGraph (Python, 그래프 기반 멀티 에이전트)
- AutoGen (Microsoft, 멀티 에이전트 대화)
- Mastra (TS, 워크플로 + 에이전트)
- Letta/MemGPT (메모리 + 에이전트)

상세 분석은 PR/RFC로, 이 파일은 1차 참조 카탈로그로 유지.
