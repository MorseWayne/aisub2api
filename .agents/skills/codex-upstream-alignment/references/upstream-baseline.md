# Reviewed Upstream Baseline

- Codex repository: `https://github.com/openai/codex.git`
- Reviewed commit: `b9559ff737000a4fcf7a0e12c3353cca1ea4d008` (`Update the openai-docs skill for GPT-6.2 (#62447)`)
- Reviewed at: `2026-11-19` (`Asia/Shanghai`)
- Previous reviewed commit: `ee344ca9458351a6ea102795fa73110ad3ede5dc`
- Analyzed range: `ee344ca9458351a6ea102795fa73110ad3ede5dc..b9559ff737000a4fcf7a0e12c3353cca1ea4d008` (269 commits, 1,692 changed files screened by contract relevance)
- Reviewed for aisub2api: `v6.0.9`, commit `bd718581194039c75a03891e66fa64529d4446d7`.
- Review scope: GPT-6.2 catalogs and reasoning effort, scoped spend-limit errors, terminal SSE delivery, Apps tool result pass-through and authorization, WebSocket and compaction lifecycle, and endpoints outside the proxy contract.
- Review report: [2026-11-19 Codex upstream alignment](../../../../docs/reviews/2026-11-19-codex-upstream-alignment.md). Previous review: [2026-11-15](../../../../docs/reviews/2026-11-15-codex-upstream-alignment.md).
- P1 and P2 were implemented against this baseline in the working tree. See the implementation results and remaining verification boundaries below.

## Checkout Synchronization

- Fetched target: `origin/main`, resolved to `b9559ff737000a4fcf7a0e12c3353cca1ea4d008` before resetting.
- Local checkout `before_sha`: `ee344ca9458351a6ea102795fa73110ad3ede5dc`.
- Local checkout `after_sha`: `b9559ff737000a4fcf7a0e12c3353cca1ea4d008`.
- Last completed synchronization: `2026-11-19` (`Asia/Shanghai`).
- Ran `git reset --hard <fetched_sha>` and `git clean -fd` in the identified Codex reference checkout. The checkout was clean before synchronization; no local tracked edits or untracked files were replaced. HEAD matches the fetched target and status is clean.
- No upstream source edits, commits, pushes, binary installs, or service restarts were performed.

## Known Contract

- Native `/v1/responses` remains streaming-only and stateless for OpenAI and ChatGPT provider types. Downstream WebSocket probes return 426.
- OpenAI native requests preserve standard and unknown fields. ChatGPT applies provider-specific input, output-token, reasoning, and Responses Lite normalization; `bd71858` also removes unsupported `temperature` and `top_p` and advertises those limitations.
- Native items preserve incremental tools, configuration updates, numeric reasoning, item IDs, image references, encrypted output, annotations, logprobs, client metadata, and unknown future fields.
- `/v1/models` exposes aisub2api capability metadata rather than Codex's complete private model catalog schema.
- ChatGPT fallback models include GPT-6.2 Sol and GPT-6.1 Sol/Terra at 500,000 tokens and GPT-6.2 Astra at 272,000. Online catalogs and explicit overrides take precedence.
- Native Responses preserves upstream HTTP error codes and request IDs and forwards terminal SSE `error` and `response.failed` events. Scoped spend limits are terminal HTTP 429 errors.
- Automatic WebSocket fallback after connection or protocol startup errors is one-way for the provider instance. An advertised size limit or an early 1009 falls back for the individual request.

## Implemented Alignment (2026-11-19)

- Preserved structured native HTTP and SSE errors, including retry advice and the original native 400/403/429 status for the reviewed cases. Classified the six terminal codes from the November 15 review and scoped spend limits. An explicit 503 context error retains overload semantics, and statusless context errors retain existing recovery behavior.
- Added the two GPT-6.2 fallback entries and updated GPT-6.1 Sol. GPT-6.2 Ultra resolves to `xhigh`. The fallback uses the catalog's Lite hints, with explicit user overrides taking precedence. The existing fallback recommendation remains GPT-5.6 Sol.
- Added 1 MiB upstream WebSocket fragmentation and respected message limits carried with cached connections. Auto fallback for size applies per request before any server text on a 1009. Forced WebSocket surfaces the size error directly.
- Passed 27 local protocol probes on the newly built executable. All 694 Cargo tests passed, including the existing zstd retry behavior and new HTTP, SSE, and WebSocket regressions. Formatting and Clippy with `-D warnings` passed.

## Pre-Implementation Findings (2026-11-19)

- Ran 22 isolated loopback probes using the installed aisub2api `6.0.9` binary. Used synthetic credentials, separate config and cache directories, and local fixtures. No real provider or production deployment was used. This review updates documentation and the baseline. Existing uncommitted runtime changes were outside the runtime test scope.
- Confirmed published-catalog parsing for GPT-6.2 Sol (500K), GPT-6.2 Astra (272K), and GPT-6.1 Sol (500K), using a loopback server with the upstream bundled catalog. The built-in proxy fallback omits both GPT-6.2 models and reports GPT-6.1 Sol at 272K.
- Confirmed GPT-6.2 Sol native requests forward `reasoning.effort=ultra` with both remote and fallback discovery. The bundled Codex catalog resolves Ultra to `xhigh` at request construction. Online discovery enables the Lite-specific `reasoning.context=all_turns`; the fallback probe omits it.
- Confirmed body-only spend limits for API key, project, and organization each made three requests when OpenAI HTTP 429 retries were enabled. The API key code was mapped to retryable generic 429; project and organization 429s became generic HTTP 400.
- Confirmed native SSE `error` was re-encoded as `upstream_stream_error`, while `response.failed` preserved the error object and nested retry advice. Both terminated before a subsequent synthetic completion. This includes the newly recognized statusless policy errors and scoped spend limit errors.
- Confirmed the OpenAI and ChatGPT native request paths preserve synthetic Apps result fields. Codex supplies private result bodies only to authorized ChatGPT destinations. Real-provider acceptance and entitlement are unverified.

## Verification and Findings (2026-11-15)

- Review-only update: no aisub2api runtime source changes, commits, pushes, deployment updates, or real-provider calls in this review.
- Ran 22 isolated loopback protocol probes against the existing `bd71858` release build with synthetic credentials and independent config/cache/Claude settings directories. The temporary fixture processes were stopped after use.
- Confirmed gaps: six new terminal codes become generic translated errors; native `error` loses its original code while native `response.failed` preserves it; three HTTP 403 codes and HTTP 400 `context_length_exceeded` lose JSON machine codes; a streamed context error carrying status 503 is incorrectly classified as request-too-large.
- Confirmed native preservation of prompt-cache TTL, explicit null request-country, access programs, mixed incremental input, allocated item IDs, file-backed audio references, numeric reasoning, and hosted tool declarations. This proves passthrough, not private-backend entitlement or acceptance.
- Static gaps: upstream WS requests need bounded framing and handshake message-limit handling; optional reasoning-removal statistics are absent. Existing early-close Auto fallback partially covers size failures.
- No bundled `models.json` changes occurred in the reviewed range. Current incremental-tools routing was checked at the target revision rather than inferred from the initial PR defaults.
- Documentation diff checks passed. No runtime code changed, so the full Cargo suite was not rerun for this review. The test counts below belong to the earlier implementation.

## Implemented Alignment (2026-11-03)

- Added `gpt-6.1-terra` fallback metadata: 500K context, image input, Responses Lite, low through ultra reasoning aliases, and priority service-tier capability.
- Recognize `model_not_found` in HTTP 400/403/404 and statusless streamed/WebSocket errors while preserving the provider message.
- Recognize six audio validation codes as terminal invalid requests across all transports.
- Added `PolicyCapacityUnavailable`, preserving `Retry-After`, telemetry attribution, and retryable downstream 529 behavior without changing the requested model or service tier.
- Removed the default-off WebSocket prewarm path, standalone-tool rollback mode, provider-level boolean Responses Lite parsing, and timed WebSocket recovery state. ChatGPT requests now use one current tool shape and one-way automatic HTTP fallback.

## Historical Verification (2026-11-03)

- `cargo fmt --all` and `git diff --check` passed.
- `cargo test --locked --workspace --all-targets -- --quiet`: 618 passed (43 CLI, 32 config, 11 core, 401 provider, 112 server unit, 19 server integration tests).
- `cargo clippy --locked --workspace --all-targets -- -D warnings` passed.
- Local tests used loopback mocks and synthetic errors. No real-provider tests were performed.

## Remaining Verification Boundaries

- P3 is optional: count reasoning removals from `input_transformations` without storing encrypted contents or making diagnostic parsing fatal. Actual private-backend availability remains unverified.
- Real-provider entitlement, model lane and service-tier behavior, policy capacity failures, model-not-found variants, audio validation errors, and private Apps result acceptance remain unverified.
- Codex's permanent HTTP fallback is session scoped while aisub2api applies it to the shared provider instance. This intentionally favors a single simple transport state; a failing client can move unrelated clients on the same provider to SSE until reload.
- Active upstream interruption on Lite cancellation is a disabled Codex runtime feature and is not exposed by aisub2api.
- Guardian, gateway/workspace orchestration, model-owned prompt/tool templates, voice/WebRTC, image generation/file upload endpoints, remote exec-server, and app-server APIs remain outside the public proxy scope.

## Updating This File

Advance this baseline only after reviewing relevant upstream changes and verifying requested implementation. Keep synchronization separate from review completion and preserve unresolved verification boundaries.
