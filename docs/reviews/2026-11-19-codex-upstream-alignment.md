# 2026-11-19 Codex 上游同步与适配审查

优先修复原生 Responses 的错误码和重试语义。同一轮补齐 ChatGPT 的 GPT-6.2 回退目录与推理档映射。Apps 工具结果字段已可透传，生成和转发私有结果仍由 Codex 的目标校验与授权控制。

## 同步与审查范围

- 官方 `openai/codex` 的 `origin/main` 从 `ee344ca9458351a6ea102795fa73110ad3ede5dc` 同步到 `b9559ff737000a4fcf7a0e12c3353cca1ea4d008`。目标在 `2026-11-19T06:34:53Z` 提交，本地 HEAD 与远端跟踪分支一致。同步前后工作树均干净。 [上游比较](https://github.com/openai/codex/compare/ee344ca9458351a6ea102795fa73110ad3ede5dc...b9559ff737000a4fcf7a0e12c3353cca1ea4d008)
- 筛查 269 个提交和 1,692 个变更文件，核查 Responses、SSE/WebSocket、模型目录、工具结果和认证边界。代理仓库为 `/home/wayne/source/open/claude-proxy`，Git 远端为 `MorseWayne/aisub2api`，审查基础为 `bd718581194039c75a03891e66fa64529d4446d7`，版本 `6.0.9`。
- 回环使用本机已安装的 `aisub2api 6.0.9`。源码对照纳入当前工作树，得出的结论标明了实测与推断。工作树已有的 ChatGPT zstd 拒绝后重试改动未纳入本轮运行验证。

## 对齐矩阵

| 上游变化 | aisub2api 当前行为 | 跟进与验证 | 上游证据 |
| --- | --- | --- | --- |
| `api_key_spend_limit_exceeded`、`project_spend_limit_exceeded`、`organization_spend_limit_exceeded` 在 HTTP 429 和无状态流事件中保留各自的终止含义 | OpenAI 路径在配置重试 HTTP 429 时先按状态和响应头重试。三个机器码只出现在响应体时均尝试三次；最终 API key 上限变成可重试的 `rate_limit_exceeded`，项目和组织上限变成无机器码的 HTTP 400。流翻译已将项目、组织两项归为无效请求，API key 上限仍进通用错误分支 | **P1，已复现。** 在重试前检查有界响应体中的终止码，保留原生 429、原码和请求 ID，保证同类错误只发一次；普通 429 继续遵守既有重试配置。翻译路径维持现有协议的错误形状与状态兼容性 | [支出上限分类](https://github.com/openai/codex/commit/cc7a62e19cb4703495e3a6d01e1af076e8223833) |
| Codex 立即返回已识别的 SSE 错误。无状态 `bio_policy`、`cyber_policy` 等属于终止错误；`policy_capacity_unavailable`、`server_is_overloaded` 可携带退避信息；Codex 的解析器继续跳过未知的通知型 `error` | 代理会立即终止，但原生 SSE 将 `error` 重编码为顶层 `upstream_stream_error`，替换机器码并丢失结构化退避信息。`response.failed` 保留嵌套错误及其头字段。本机验证两种事件后均没有继续输出成功。上游错误语义依赖事件形状，不能用一个通用文本替代 | **P1，已复现。** 原生路径保留上游 `error` 的形状、机器码及兼容的退避信息，继续保留 `response.failed`。回归覆盖终止时序、带状态的上下文错误、未知事件和不同上游的错误形状；Codex 跳过未知通知的行为不宜推广到所有 provider | [策略错误](https://github.com/openai/codex/commit/c5cff4335c1e4675227c8b89520aaf09564b9aa7) · [立即报告流错误](https://github.com/openai/codex/commit/513577302a9aacd217b128a701702a54b7e752b6) |
| 内置目录新增 `gpt-6.2-sol`（500K，首选）与 `gpt-6.2-astra`（272K），并将 `gpt-6.1-sol` 的活跃窗口升到 500K。三者目录里的可选最大窗口为 872K。两款 6.2 的最低 Codex 版本标为 `0.166.0`，客户端依据 `multi_agent_reasoning_effort` 将 Ultra 选项解析为请求值 `xhigh` | 模拟线上目录正确展示三款模型与活跃窗口，并将 6.2 Sol 标为默认。内置回退只显示 6.1 Sol 的 272K。原生 6.2 Sol 请求线上命中时添加 `reasoning.context=all_turns`，目录不可用时未添加。两种情况下直接请求都可被路由，且最终都向模拟上游发送 `effort=ultra`，因为两款新型号缺少代理本地推理档别名支持 | **P1，已复现目录及 Sol 请求差异。** 补齐回退元数据、6.1 Sol 活跃窗口和新型号推理档映射。Ultra 对齐应考虑 6.2 的 `xhigh`，现有型号的 `max` 行为单独保留。两款 6.2 的目录均提示 Lite，实际 Full、Lite 和并行工具待真实授权账号验证。保留显式模型别名和覆盖的优先级 | [GPT-6.2 目录与默认值](https://github.com/openai/codex/commit/6c0cee502d215e5df2dd9a303cf2e252229c9e12) · [客户端推理档解析](https://github.com/openai/codex/blob/b9559ff737000a4fcf7a0e12c3353cca1ea4d008/codex-rs/protocol/src/openai_models/reasoning_effort.rs) |
| Apps 工具结果增加 `tool_result`，嵌套调用还可附上 `tool_result_call_id`。当前 Codex core 在常规 turn 的 HTTP 请求路径填充正文；要求可信 ChatGPT origin、匹配且仍有效的 auth owner、线程授权，并拒绝带 provider query 参数的自定义目的地。传输层保留原始 trace 正文，遇到会继续发送请求体的重定向时使用原始请求重试 | OpenAI 和 ChatGPT 原生请求均原样转发合成的 `tool_result_metadata`、`tool_result`、`tool_result_call_id`。代理透明传输调用方给出的 JSON，未参与 Codex 内部授权。在 Codex 中用自定义代理 URL 时，上游证据显示私有增强正文会被目的地判定排除 | **已覆盖原生字段，授权功能不适用。** 维持通用透传。保留 Codex 的目标验证、重定向与 trace 边界，不为自定义地址添加伪装、自动生成正文或扩大授权 | [字段支持](https://github.com/openai/codex/commit/f1b69864161866a111610c4b4351d397fb644b6f) · [授权与传输](https://github.com/openai/codex/commit/d1d1ba0b837d34f314b7f469896e8babbc91f949) · [嵌套结果](https://github.com/openai/codex/commit/b5d9f7116e256e59f4a036c1c3e18979994598c4) · [query 限制](https://github.com/openai/codex/commit/770c38d0b16efc78e7a1e77868605ab457b26401) |
| WebSocket 启动预热保存 trace；远端压缩在子 agent 中断时保留已有摘要 | 代理使用自己的上游 WS 生命周期，原生 Responses 采用 SSE。上游启动预热的 trace 修复和中断时的摘要持久化发生在 Codex 客户端。此前记录的上游 WS 分帧和握手上限读取缺口仍存在 | **预热与摘要逻辑不适用。** 继续按 11-15 记录验证大消息及早期 1009 的降级边界 | [预热 trace](https://github.com/openai/codex/commit/7457b7268fe109e5387bf0eac80ef4acd7c31365) · [中断保留摘要](https://github.com/openai/codex/commit/f8e58064fccfdf9c220996bdb26d081a6a732d9f) |
| Bedrock Astra 支持 `ultrafast`，Vertex 目录加入两款模型；现有 Realtime 通话支持外部头认证；音频听写增加 multipart `/transcribe`；工具调用归因头只用于指定执行器的出站 HTTP/CONNECT 代理 | 当前适配器不承担 Bedrock、Vertex、Realtime、音频听写或执行器出站代理。ChatGPT 模型目录中的 `priority` 是独立服务档 | **不适用当前接口。** 这些变化不构成扩大公开端点、服务档或转发执行器身份头的依据 | [云端目录](https://github.com/openai/codex/commit/6c0cee502d215e5df2dd9a303cf2e252229c9e12) · [Realtime 认证](https://github.com/openai/codex/commit/8dcd56d0cb84fa8220c6056d3ecdb861b7b7b61c) · [音频听写](https://github.com/openai/codex/commit/83c46d0a58950c63f1ceff9a79ccb31bc612ec07) · [工具归因](https://github.com/openai/codex/commit/457a191479f15ce517d70e9a7bc4caf9ddd6d404) |

## 本机验证

模拟上游和代理使用独立回环端口及临时配置和缓存。凭据均为合成数据，结束后临时进程和配置已清理。目录响应取自同步后的 Codex `models.json`，目录不可用通过返回 404 模拟。未访问真实 provider 或运行真实 Codex 重试循环。

| 场景 | 用例数 | 结果 |
| --- | ---: | --- |
| `/v1/models` 的线上目录与内置回退 | 2 | 线上正确展示 500K / 272K / 500K；回退缺少两款 6.2，6.1 Sol 为 272K |
| 三种支出上限的原生 HTTP 429 | 3 | 模拟响应仅在 `error.code` 提供上限类型。启用 `rate_limits=true` 时各尝试三次，随后向客户端丢失原始支出上限码 |
| 七种错误分别通过原生 SSE `error` 与 `response.failed` 注入 | 14 | `error` 重编码，`response.failed` 保留原码。策略容量用例的嵌套 `Retry-After` 只有后者保留。两种事件后附加的成功事件均未透出 |
| 合成 Apps item 和 `reasoning.effort=ultra` | 3 | OpenAI、ChatGPT 线上目录和 ChatGPT 回退均保留三个工具结果字段并返回模拟完成事件。两条 ChatGPT 请求的 `ultra` 均未映射，只有线上目录用例添加 `reasoning.context=all_turns` |

HTTP 429 复现输入为 `{"error":{"code":"api_key_spend_limit_exceeded","message":"synthetic spend limit"}}`，响应头为 `Retry-After: 0` 和合成请求 ID，未附带 `x-openai-error-code`。SSE 容量复现输入为 `{"type":"error","error":{"code":"policy_capacity_unavailable","message":"synthetic stream result","headers":{"Retry-After":"2"}}}`。

## 实施顺序

1. 修复原生 Responses 终止错误保真与 HTTP 重试判定，合并上轮错误语义缺口。覆盖首个失败事件、上游状态约束、机器码、请求 ID 和退避语义。确认真正的限流仍会重试，失败事件后不产生成功终态。 [11-15 审查](2026-11-15-codex-upstream-alignment.md#建议跟进顺序) · [HTTP 重试判断](../../crates/aisub2api-providers/src/http.rs#L538) · [Responses 错误分类](../../crates/aisub2api-providers/src/responses/errors.rs#L11) · [下游 SSE 编码](../../crates/aisub2api-server/src/downstream.rs#L1384)
2. 补齐 GPT-6.2 Sol/Astra 的内置目录、GPT-6.1 Sol 的回退活跃窗口与新型号推理档映射。回归线上目录、内置回退及用户覆盖，使用已授权真实账号确认两款模型的 Full/Lite 和工具调用再制定默认策略。 [内置目录](../../crates/aisub2api-providers/src/chatgpt.rs#L3720) · [推理档别名](../../crates/aisub2api-providers/src/chatgpt.rs#L4013)
3. 延续上轮上游 WS 分帧与握手尺寸验证。推理回放诊断按实际排障需求跟进。

审查阶段使用已安装的 6.0.9 二进制完成 22 组本机探测。下方记录实施后的行为及验证。

## 实施结果

- 原生 Responses 保留 SSE `error` 与 `response.failed` 的形状、机器码和嵌套退避信息，失败后结束流。原生 HTTP 保留上游机器码及请求 ID。三个支出上限保留 HTTP 429 和 `x-should-retry: false`，共用 HTTP 重试路径在读取有界响应体后终止重试。普通限流仍遵循重试预算。
- 翻译路径识别支出上限及预算、推理钩子、访问计划和内容策略的终止错误。无状态或 HTTP 400 上下文错误保留上下文溢出语义，显式 503 按上游不可用处理。HTTP 403 的工作区错误和 HTTP 400 的上下文错误在原生 Responses 保留状态与机器码。
- ChatGPT 内置目录新增 GPT-6.2 Sol 500K 和 Astra 272K，并更新 GPT-6.1 Sol 至 500K。两款 GPT-6.2 的 Ultra 发送 `xhigh`。Auto 路径使用目录的 Lite 提示，用户显式覆盖优先。内置目录仍将现有 GPT-5.6 Sol 标为默认。
- ChatGPT 上游 WebSocket 使用最多 1 MiB 的帧，按实际序列化字节检查握手声明的整条消息上限。超限或首条服务端文本前收到关闭码 1009，Auto 模式为当前请求回退 SSE。收到文本后结束该失败请求，避免重放。强制 WebSocket 模式直接返回尺寸错误。

使用重新编译的二进制通过 27 组隔离回环探测，覆盖线上目录与回退、支出上限及普通限流的请求次数、HTTP 400/403 机器码、原生 SSE 终态，以及 Apps 字段和 GPT-6.2 请求推理档。Cargo 回归另外覆盖 WebSocket 帧边界上的 UTF-8 重组、握手上限和 1009 的模式与时序。GPT-6.2 的账号权限、实际 Full/Lite 与服务档和 Apps 私有结果的后端接受情况仍待真实环境验证。
