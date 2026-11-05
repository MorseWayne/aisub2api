# Anthropic Messages 流式完成边界修复

日期：2026-11-05。基线为 `2603512`，与此前的 effort 日志修复一并纳入 v6.0.4。目标：避免不完整的原生 Messages 流被当作成功结束，同时正确处理协议已完成后的 HTTP 关闭。

## 已确认的问题

OpenAI Chat Completions 和 Responses 的上游适配器已有终结检查；Anthropic 和 Copilot 的原生 Messages SSE 接收逻辑则重复实现，均把 HTTP EOF 当作普通结束。

server 另有 tool-use 终结 watchdog，但普通文本/thinking 不触发它。结果是：

- Anthropic 普通文本流提前结束时，客户端只能看到没有终结事件的半段消息，代理本身没有报错。
- 将相同输入流转换为 Chat Completions 时，编码器的正常 finish 路径可能补出 `finish_reason = stop`，让部分内容表现为成功完成。
- 已收到完整 `message_stop` 后，适配器仍继续等待 HTTP 结束；后续传输错误可能被当作本次回复失败。

新增 HTTP 集成测试在修改前失败：`/v1/messages` 收到 `PARTIAL_ONLY`，随后响应体结束，缺少错误事件。

## 当前完成契约与实现

一个上游 Messages 请求对应一条回复流。`message_stop` 表示消息完成；流内 `error` 是另一种终止事件，需要按原形转发。HTTP EOF 本身不表示成功。

将两处原生 Messages 接收逻辑收敛到 `sse::stream_anthropic_response`，由 Anthropic provider 和 Copilot 的 Messages 路径共同调用：

1. 逐帧转发，保留已有帧大小限制、读取错误处理和通道背压。
2. 解析到 `message_stop` 或原生 `error` 后，发送该事件并结束，不继续等待 HTTP EOF。
3. 在没有终止事件时结束，返回 `ProviderError::Network`，消息为 `upstream Anthropic stream ended before message_stop`；空的 HTTP 200 流同样报错。
4. 如果最后一个完整帧没有空行分隔符，在 EOF/传输错误时尝试解析缓冲尾帧。尾帧确为终止事件时正常结束；否则继续报告错误。
5. 不伪造 `message_stop`，不重写原生 API error payload，不增加配置、重试机制或缓存。

帧解析规则沿用原实现，同时支持 SSE `event` 名和 JSON `type` 标识终止事件。这是完成边界检查，不是完整的事件序列、JSON 字段或内容块生命周期验证。

server 已开始发送 SSE 时，HTTP 状态仍然为 200；失败由既有错误事件表示。对 Chat Completions，部分文本后会出现错误 envelope，不再走正常完成分支补出成功 finish reason。

## 验证

### 自动回归

- 原生文本和 thinking 的 `content_block_stop` 不能代替 `message_stop`。
- 空成功响应必须失败。
- 无空行的终结尾帧，在模拟 HTTP 长度错误下仍可恢复。
- 收到终结事件后，即使模拟服务器保持连接，也在测试期限内结束。
- 原生 API error 内容保持原形，不追加一个因缺少 `message_stop` 而产生的假错误。
- HTTP 集成测试覆盖 `/v1/messages` 和 `/v1/chat/completions`：部分文本被保留，错误可见，没有成功的 `finish_reason = stop`。

`cargo test --locked -p aisub2api-providers -p aisub2api-server -- --quiet`：547 项通过（providers 410、server unit 115、server integration 22）。该数量包括此前 effort 日志修复的测试。

`cargo clippy --locked --workspace --all-targets -- -D warnings`、`cargo fmt --all --check`、`git diff --check`、`cargo build --locked -p aisub2api-cli` 均通过。

### 真实客户端与模拟上游

固定 Claude Code 2.1.318，隔离配置、虚构凭据，仅连接本地回环接口。五组场景全部退出 0 并收到完成标记：

| 场景 | 上游请求数 | 结果 |
| --- | --- | --- |
| 普通回复 | 1 | 正常完成 |
| Read 工具调用 | 2 | 参数分段、工具回传和最终回复完成 |
| thinking | 1 | 既有 thinking/text fixture 成功消费 |
| 首次文本截断，第二次返回完整回复 | 2 | 客户端重试后完成 |
| 完整消息后模拟 HTTP 长度不符并关闭连接 | 1 | 完成，没有额外重试 |

首次截断场景的隔离 SQLite 指标记录为 `(is_error=1, terminal_reason=stream_error)`，重试成功后另有 `(is_error=0, terminal_reason=completed)`，不再把第一次的部分内容记成成功回复。

上述结果验证本地协议行为；不证明所有 Claude Code 版本都会自动重试，也未验证真实 Copilot 网关、模型质量或生产网络性能。原生 API error 的内容透传不等同于对其全部指标分类进行改造。

## 本地证据与参考

临时目录 `/tmp/aisub2api-rea-20261105` 中保留：

- `probe_streams.py`、`stream-probe-after.jsonl`：实验脚本与五组摘要。
- `captures-streams-proxy/<case>/requests.json`、`stdout.jsonl`：请求和客户端输出。
- `captures-streams-proxy/truncated-once/proxy-config/aisub2api/metrics.db`：截断和恢复的指标记录。

临时文件不加入仓库。外部协议依据为 Anthropic 的 [Messages streaming](https://platform.claude.com/docs/en/build-with-claude/streaming) 和 [gateway streaming guide](https://code.claude.com/docs/en/llm-gateway-protocol#streaming)。
