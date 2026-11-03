# Reviewed Upstream Baseline

- Codex repository: `https://github.com/openai/codex.git`
- Reviewed commit: `fbf5f0d3a716d08c254d3e4941d5b12eaff5afe7` (`Decode live transcript updates in the cloud client (#59047)`)
- Reviewed at: `2026-11-03` (`Asia/Shanghai`)
- Previous reviewed commit: `e790ce271749ce431e446a9e2cc66485fe60f931`
- Analyzed range: `e790ce271749ce431e446a9e2cc66485fe60f931..fbf5f0d3a716d08c254d3e4941d5b12eaff5afe7` (731 commits)
- Reviewed for aisub2api: `v6.0.0`; review started from `fd39f4c` (`v5.0.0`).
- Review scope: Responses HTTP/SSE/WebSocket error contracts, ChatGPT fallback models, prompt-cache/session identity, incremental tools, native item preservation, transport fallback, cancellation, WebSocket probes, model metadata, and exclusion of client-only runtime changes.
- Review report: [2026-11-03 Codex upstream alignment](../../../../docs/reviews/2026-11-03-codex-upstream-alignment.md).

## Checkout Synchronization

- Fetched target: `origin/main`, resolved to `fbf5f0d3a716d08c254d3e4941d5b12eaff5afe7` before resetting.
- Local checkout `before_sha`: `e790ce271749ce431e446a9e2cc66485fe60f931`.
- Local checkout `after_sha`: `fbf5f0d3a716d08c254d3e4941d5b12eaff5afe7`.
- Last completed synchronization: `2026-11-03` (`Asia/Shanghai`).
- Ran `git reset --hard <fetched_sha>` and `git clean -fd` in the identified Codex reference checkout. The checkout was clean before synchronization; no local tracked edits or untracked files were replaced. HEAD matches the fetched target and status is clean.
- No upstream source edits, commits, pushes, binary installs, or service restarts were performed.

## Known Contract

- Native `/v1/responses` remains streaming-only and stateless for OpenAI and ChatGPT provider types. Downstream WebSocket probes return 426.
- OpenAI native requests preserve standard and unknown fields. ChatGPT applies provider-specific input, output-token, reasoning, and Responses Lite normalization.
- Native items preserve incremental tools, configuration updates, numeric reasoning, item IDs, image references, encrypted output, annotations, logprobs, client metadata, and unknown future fields.
- `/v1/models` exposes aisub2api capability metadata rather than Codex's complete private model catalog schema.
- ChatGPT fallback models now include `gpt-6.1-terra` with the Codex catalog's 500,000-token active context. Online catalogs and explicit overrides retain priority.
- `model_not_found`, audio validation, Flex, policy capacity, throttling, quota, and overload errors retain distinct terminal/retry semantics across HTTP, SSE, and WebSocket paths.
- Automatic WebSocket fallback is one-way for the lifetime of the provider instance. Legacy timed recovery and `generate=false` prewarm were removed.

## Implemented Alignment (2026-11-03)

- Added `gpt-6.1-terra` fallback metadata: 500K context, image input, Responses Lite, low through ultra reasoning aliases, and priority service-tier capability.
- Recognize `model_not_found` in HTTP 400/403/404 and statusless streamed/WebSocket errors while preserving the provider message.
- Recognize six audio validation codes as terminal invalid requests across all transports.
- Added `PolicyCapacityUnavailable`, preserving `Retry-After`, telemetry attribution, and retryable downstream 529 behavior without changing the requested model or service tier.
- Removed the default-off WebSocket prewarm path, standalone-tool rollback mode, provider-level boolean Responses Lite parsing, and timed WebSocket recovery state. ChatGPT requests now use one current tool shape and one-way automatic HTTP fallback.

## Verification

- `cargo fmt --all` and `git diff --check` passed.
- `cargo test --locked --workspace --all-targets -- --quiet`: 618 passed (43 CLI, 32 config, 11 core, 401 provider, 112 server unit, 19 server integration tests).
- `cargo clippy --locked --workspace --all-targets -- -D warnings` passed.
- Local tests used loopback mocks and synthetic errors. No real-provider tests were performed.

## Remaining Verification Boundaries

- Real-provider entitlement and behavior for `gpt-6.1-terra`, policy capacity failures, model-not-found variants, and audio validation errors remain unverified.
- Codex's permanent HTTP fallback is session scoped while aisub2api applies it to the shared provider instance. This intentionally favors a single simple transport state; a failing client can move unrelated clients on the same provider to SSE until reload.
- Active upstream interruption on Lite cancellation is a disabled Codex runtime feature and is not exposed by aisub2api.
- Guardian, gateway/workspace orchestration, model-owned prompt/tool templates, voice/WebRTC, image generation/file upload endpoints, remote exec-server, and app-server APIs remain outside the public proxy scope.

## Updating This File

Advance this baseline only after reviewing relevant upstream changes and verifying requested implementation. Keep synchronization separate from review completion and preserve unresolved verification boundaries.
