# 2026-11-03 Codex 上游对齐

## 同步与审查范围

- 上游仓库：`https://github.com/openai/codex.git`，分支 `main`，跟踪 `origin/main`。
- 同步前及上次审查基线：`e790ce271749ce431e446a9e2cc66485fe60f931`。
- 本次 fetch 后同步到：`fbf5f0d3a716d08c254d3e4941d5b12eaff5afe7`，`Decode live transcript updates in the cloud client (#59047)`。
- 实际审查范围：`e790ce271..fbf5f0d3a`，共 731 个提交。审查优先覆盖 `codex-api`、Responses HTTP/SSE/WebSocket、模型目录、错误分类、请求元数据和原生 item；UI、沙箱、Guardian、app-server 与本地运行时变化只在影响代理线协议时纳入。
- Codex 参考仓库按项目 skill 执行 `git reset --hard <fetched_sha>`、`git clean -fd`。同步前工作区干净，没有本地 tracked/untracked 内容被替换；同步后 HEAD 与 `origin/main` 一致且状态干净。
- aisub2api 从 `fd39f4c`、版本 `5.0.0` 开始本轮跟进；对齐与裁剪纳入 `v6.0.0`。原有 `.cargo/`、`.claude/settings.json` 未修改。

## 对齐矩阵

| 上游证据 | 线协议变化 | aisub2api 现状 | 状态与处理 | 兼容与验证 |
| --- | --- | --- | --- | --- |
| [126a0ee92](https://github.com/openai/codex/commit/126a0ee92)，`models-manager/models.json` | 新增 `gpt-6.1-terra`：500K active context、872K max、Lite、图像输入、完整 reasoning 档位、priority tier | 线上模型目录可发现，但内置 fallback 缺失 | 已修复：加入 500K fallback，并纳入 reasoning alias/Lite 请求构建 | 不修改默认模型；真实账户可见性及 500K entitlement 未实测；fallback 测试通过 |
| [60adfa851](https://github.com/openai/codex/commit/60adfa851) | `model_not_found` 在 HTTP 400/403/404 及 `error`/`response.failed` 中均为终止错误 | 仅无条件把 HTTP 404 当模型不存在；400/403 和流事件分类不准确 | 已修复：HTTP、SSE、WS 统一分类并保留消息 | 下游返回既有 model-not-found 契约，不触发重试；三传输测试通过 |
| [6a6e2fb1f](https://github.com/openai/codex/commit/6a6e2fb1f) | 新增 `policy_capacity_unavailable`，与普通 overload 分开归因，但沿用 `Retry-After` 重试建议 | 原来落入普通 503 overload 或未知流错误 | 已修复：新增独立 provider error；三种下游协议返回 retryable 529，并保留 retry advice | 不自动换模型/档位；HTTP、SSE、WS 及下游映射测试通过 |
| [5a7353f04](https://github.com/openai/codex/commit/5a7353f04) | 六种音频校验错误在 HTTP/流事件中均为不可重试输入错误 | HTTP 400 已终止，但 statusless 流错误会成为通用 upstream error | 已修复：统一映射为 `InvalidRequest` | aisub2api 仍声明 audio input unsupported；只改善错误语义，不新增音频能力 |
| [ab6095dde](https://github.com/openai/codex/commit/ab6095dde) | `prompt_cache_key` 可独立于 session/thread identity 自定义或省略 | 显式 `prompt_cache_key` 已优先于稳定会话 key；continuation identity 单独计算；原生请求透传 | 已对齐，无代码修改 | 既有 prompt-cache 与 continuation 测试覆盖 |
| [c953dacb4](https://github.com/openai/codex/commit/c953dacb4)、[f088e0c4f](https://github.com/openai/codex/commit/f088e0c4f)、[e3149ddb1](https://github.com/openai/codex/commit/e3149ddb1) | Lite 默认增量 tools、轮次间动态工具、用户输入 item ID | 原生输入保留未知 item/id/顺序；Messages Lite 已生成 `additional_tools` 前缀 | 已对齐，无代码修改 | 不把 Codex 客户端的工具调度状态复制到代理 |
| [446044992](https://github.com/openai/codex/commit/446044992)（取代 [0a2dda4b5](https://github.com/openai/codex/commit/0a2dda4b5)） | Codex session 一旦 WS fallback，余下 session 固定 HTTP，并删除恢复 probe/cooldown | aisub2api 原来维护 120 秒全局 cooldown、恢复重试和旧 `generate=false` prewarm 分支 | 已简化：provider 实例单向切换到 SSE，重载 provider 后恢复；删除 cooldown 与 prewarm 状态机 | 共享 provider 的 fallback 范围比单个 Codex session 大，但状态、定时恢复和额外上游请求全部移除；连续请求测试覆盖 |
| [f276b8218](https://github.com/openai/codex/commit/f276b8218) | 可选 `interrupt_on_cancel` 主动中断 Lite 上游推理 | aisub2api 在 consumer 消失时关闭/释放上游，未发送 Codex agent 的 interrupt 控制 | 不适用当前公开契约 | 上游功能默认关闭，属于 Codex turn runtime；代理没有对应会话控制 API |
| [b538d2b34](https://github.com/openai/codex/commit/b538d2b34) | 增加 WS handshake probe 证据与诊断脱敏 | 下游 GET `/v1/responses` 返回 426，引导 Codex 使用 HTTP；上游 ChatGPT WS 有自己的 fallback | 已对齐/诊断不适用 | 不新增 Codex doctor/probe API；现有 426 集成测试保留 |
| [7656a23b8](https://github.com/openai/codex/commit/7656a23b8)、[0d6838318](https://github.com/openai/codex/commit/0d6838318) | 失败推理 trace 保留 incomplete output；持久化 image generation ID | 原生 Responses 事件和未知字段保持无损；翻译路径未声明 image output 能力 | 已对齐或不适用 | 不为未暴露的 image generation/file upload 端点新增兼容声明 |
| [b26f68267](https://github.com/openai/codex/commit/b26f68267)、[16ddd538b](https://github.com/openai/codex/commit/16ddd538b) | 模型目录新增 search provider UI 元数据；app-server completion 通知新增 assistant content token 计数 | 代理模型目录使用自身 capability schema，native usage/event 保持未知字段 | 不适用公开接口 | 不复制 Codex UI/app-server schema |

## 设计裁剪

- 删除默认关闭的 `websocket_prewarm` 请求、continuation 特例、失败 fallback 和对应测试夹具；当前实现不再发送 `generate=false`。
- 删除 `standalone_tools=false` 回滚分支，ChatGPT 始终使用当前 Codex standalone/custom-tool shape。
- 删除 provider 级 `responses_lite` 的布尔和别名解析，只保留 `auto/on/off` 三个明确值；per-model capability override 仍使用布尔能力值。
- 删除 120 秒 cooldown、恢复时间戳和恢复后重试；Auto transport 只保留 `Auto → SSE` 单向状态。
- 删除已失效的 standalone-tools 历史设计稿。`crates/` 下本轮净减少 457 行，减少 5 个仅服务于已删除分支的测试，行为覆盖转向当前单一路径。

## 验证

- `cargo fmt --all`、`cargo check --workspace --all-targets`、`git diff --check`：通过。
- `cargo test --locked --workspace --all-targets -- --quiet`：618 项通过（CLI 43、config 32、core 11、provider 401、server unit 112、server integration 19）。
- `cargo clippy --locked --workspace --all-targets -- -D warnings`：通过。
- 测试仅使用回环模拟服务和合成错误响应；没有调用真实模型、修改凭据、重启服务、提交或推送。

## 剩余边界

- `gpt-6.1-terra` 的账户可见性、500K active context、priority tier 与 Lite 行为仍需真实账户验证；线上目录和显式 capability override 继续优先于 fallback。
- `policy_capacity_unavailable`、400/403 `model_not_found` 和音频错误均按 Codex 源码及本地模拟对齐，未在真实 ChatGPT backend 触发。
- 自动 transport 的 fallback 现在作用于共享 provider 实例；一个客户端触发后，其他客户端也会使用 SSE，直到配置重载或进程重启。这是为删除恢复状态机而接受的行为变化。
- 旧 `websocket_prewarm`、`standalone_tools` 和 provider 级布尔 `responses_lite` 配置已删除；旧字段不再提供行为兼容，`responses_lite` 必须使用 `auto/on/off` 字符串。
- Guardian、Realtime/WebRTC、voice/audio upload、image generation/file upload、远程 exec-server、app-server 和 Codex UI 仍不属于 aisub2api 的公开代理契约。
