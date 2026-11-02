# Reviewed Upstream Baseline

- Codex repository: `https://github.com/openai/codex.git`
- Reviewed commit: `e790ce271749ce431e446a9e2cc66485fe60f931` (`Use a synchronous mutex for remote metadata request tracking (#54475)`)
- Reviewed at: `2026-10-16` (`Asia/Shanghai`)
- Previous reviewed commit: `0d46c252b3f29f10bacf0ef58a17a1aa5d17ead3`
- Analyzed range: `0d46c252b3f29f10bacf0ef58a17a1aa5d17ead3..e790ce271749ce431e446a9e2cc66485fe60f931` (1,820 commits)
- Reviewed for aisub2api: `v4.0.3` (release date `2026-10-17`); review started from `c885f6d2063030ed6cbae033656615d756c634b9` (`v4.0.2`).
- Review scope: Responses HTTP/SSE/WebSocket requests and terminal states, incremental tools, numeric reasoning, image references and output replay, streamed compaction, retry advice, quota/Flex errors, ChatGPT model capabilities and explicit service tiers, shared routing cookies, auth ownership, model discovery, workspace routing, and exclusion of client-only runtime changes.
- Review report: [2026-10-16 Codex upstream alignment](../../../../docs/reviews/2026-10-16-codex-upstream-alignment.md).

## Checkout Synchronization

- Fetched target: `origin/main`, resolved to `e790ce271749ce431e446a9e2cc66485fe60f931` before resetting.
- Local checkout `before_sha`: `0d46c252b3f29f10bacf0ef58a17a1aa5d17ead3`.
- Local checkout `after_sha`: `e790ce271749ce431e446a9e2cc66485fe60f931`.
- Last completed synchronization: `2026-10-16` (`Asia/Shanghai`).
- Ran `git reset --hard <fetched_sha>` and `git clean -fd` in the identified Codex reference checkout. There were no local tracked edits or untracked files to replace. HEAD matches the fetched target and status is clean.
- The upstream revision deletes root `AGENTS.md`; it was not restored locally. No upstream source edits, commits, pushes, binary installs, or service restarts were performed.

## Known Contract

- Native `/v1/responses` is streaming-only and stateless, for OpenAI and ChatGPT provider types. Downstream WebSocket probes still return 426.
- OpenAI native requests preserve standard and unknown fields. ChatGPT normalizes string input, removes unsupported `max_output_tokens`, and enforces Lite `reasoning.context=all_turns` and `parallel_tool_calls=false`.
- Native input items preserve incremental `additional_tools`, configuration updates, numeric reasoning, image `file_id`, `partial_answer`, output ciphertext, annotations/logprobs, and client metadata.
- `/v1/models` exposes proxy capability metadata, not the full private Codex `{models: ModelInfo[]}` discovery schema. Codex's optional explicit `model_catalog_url` is not an advertised proxy interface.
- Public OpenAI Astra metadata retains the prior 1,050,000-token context and 128,000-token output declarations. New ChatGPT fallback entries do not establish public OpenAI model specs.
- Historical v4.0.1 Structured Outputs live verification remains prior evidence only; this review used synthetic credentials and local mocks.

## Implemented Alignment (2026-10-16)

- Added ChatGPT fallback capabilities for `gpt-6-sol`, `gpt-6-luna`, and `gpt-6.1-sol`, with 272,000-token default context, text/image input, Lite request shaping, reasoning aliases, and suppression of unsupported sampling controls. Online catalogs and explicit capability overrides retain priority. Existing default model selection is unchanged.
- Explicit Flex configuration survives catalogs that omit Flex. Catalog default service tiers remain UI hints; missing request/config tier does not silently select priority. Fast mode still explicitly selects priority.
- HTTP retry advice is no longer shortened to five seconds. A longer delay ends local retries and is returned to the caller. Failed Responses events preserve retry headers and request IDs; textual delay parsing is restricted to throttling errors. WebSocket connection-limit errors retain server retry advice.
- `slow_down` is rate limiting, including HTTP 503 forms. Additional credit/spend-limit and policy codes receive terminal request-error classifications. Flex capacity errors have a separate provider error and return HTTP 503 with `x-should-retry: false` before streaming; the requested tier is not changed automatically.
- Native `response.incomplete` stays intact. Translated streams report unknown/interrupted incomplete reasons as errors; established token-limit/content-filter mappings remain. Usage is observed before classification. A terminal SSE response releases the upstream read even when the server leaves the socket open.
- The per-provider `__oailb` jar is shared by HTTP and WSS, including rejected handshakes. It remains restricted to known secure ChatGPT hosts with cookie scope/expiration enforced and explicit provider cookies taking precedence.
- WebSocket connection and continuation keys now include credential ownership and configured routing headers. Existing model-cache identity logic preserves known-owner token rotation and conservatively partitions opaque credentials. Model/context caches keep the previous identity-scoped behavior and schema version 2.

## Verification

- `cargo fmt --all -- --check` and `git diff --check` passed.
- `cargo test --locked --workspace --all-targets -- --quiet`: 617 passed (43 CLI, 30 config, 11 core, 403 provider, 111 server unit, 19 server integration tests).
- `cargo clippy --locked --workspace --all-targets -- -D warnings` passed.
- Provider tests were rechecked after tightening mock socket reads and dropping a test mutex before awaiting.
- The sandbox denied local listeners and Cargo cache writes; authorized checks ran with the needed filesystem/socket permissions. No real-provider tests were performed.

## Remaining Verification Boundaries

- Real-provider operation and account entitlement for new models, Flex availability, routing-cookie performance benefits, and private workspace/network constraints were not tested live.
- Detached memory identity remains unverified: native turn/root metadata is retained, but ChatGPT fills missing session/thread headers. Do not assume backend expectations for dedicated memory requests.
- Guardian agents, gateway/workspace discovery and OAuth orchestration, model-owned prompt/tool templates, voice/WebRTC, image generation/file upload endpoints, remote exec-server, and app-server APIs are outside this proxy's exposed scope.
- The earlier deliberate GPT-5.6 default-context override remains unchanged; do not apply it to the newly added GPT-6 models.

## Updating This File

Advance this baseline only after reviewing relevant upstream changes and verifying requested implementation. Keep synchronization separate from review completion and preserve unresolved verification boundaries.
