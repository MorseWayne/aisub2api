# Claude Code 到 OpenAI Responses 的 effort 与工具调用验证

日期：2026-11-05。代码基线：`2603512`。使用固定的 Claude Code 2.1.318、隔离配置和本地 OpenAI Responses 模拟接口；未调用真实模型。

后续原生 Messages 断流与完成边界的修复见[流式完成边界调研](2026-11-05-claude-code-stream-completion.md)。

## 结论

- 现有转换路径可在受控 MCP 场景完成 ToolSearch、工具发现、目标工具调用和最终回复，本轮没有据此重做工具搜索转换或扩大 capability 声明。
- 确认一个诊断问题：客户端明确指定 `output_config.effort = low` 时，上游实际收到 `reasoning.effort = low`，日志却记为 `high`、来源为 `thinking`。
- 修复日志的值和来源识别，使标准 adaptive 请求的诊断与 Responses 转换采用相同字段读取规则。上游原本正确的 effort 参数不变。

## 实验路径

```text
真实 Claude Code → 回环 relay → 隔离 aisub2api → 本地 Responses fixture
                      ↓
                记录客户端请求
```

模拟上游还记录转换后的请求。配置使用 OpenAI provider，将 Claude Sonnet alias 路由到测试模型 `gpt-5.5`；凭据均为虚构值。日志级别为 info，关闭原始 API/SSE payload 日志。

| 场景 | 客户端请求 | 模拟上游收到 | 修复前日志 | 修复后日志 |
| --- | --- | --- | --- | --- |
| `--effort low` | adaptive thinking，`output_config.effort = low` | `reasoning.effort = low` | `high`，来源 `thinking` | `low`，来源 `explicit:output_config.effort` |
| `MAX_THINKING_TOKENS=0`，effort low | disabled thinking，effort low | `reasoning.effort = none` | `none` | `none` |
| `alwaysThinkingEnabled=false`，effort low | disabled thinking，effort low | `reasoning.effort = none` | `none` | `none` |
| 受控 MCP ToolSearch | 搜索工具、发现 schema、调用目标工具 | 三轮 Responses 请求，effort high | `high` | `high` |

四个场景的真实客户端均退出 0，并收到固定完成标记。MCP fixture 收到目标 lookup 调用。

工具搜索实验的模型响应由 fixture 决定，所以只验证请求转换、工具名关联、客户端工具执行与流式回复的本地调用链；不证明真实 OpenAI/ChatGPT 模型对该形状具有完整语义支持或 token 成本优势。本轮未连接 ChatGPT OAuth backend。

## 原因与改动

`responses::convert_reasoning` 已在 adaptive thinking 分支读取 `output_config.effort`；`openai_compat::request_reasoning_effort` 的日志路径却直接取模型默认值。两处独立读取规则产生漂移。

新增 crate 内部的 `adaptive_output_effort`，由日志和 Responses 转换共同使用。仅对 `thinking.type = adaptive` 且没有 legacy budget 的请求读取字符串 effort，与原转换分支的前置条件一致。

优先级保留为：

1. 显式 OpenAI `reasoning`。
2. 显式 `reasoning_effort`，包括模型 alias 已注入的值。
3. disabled thinking 或 legacy thinking budget。
4. adaptive thinking 的 `output_config.effort`，缺失/非字符串时使用原默认值。

本轮没有将单独出现的 `output_config.effort` 扩展为新的 API 行为，也未修改模型路由、模型能力、服务档位或 provider 默认值。日志记录的是这层请求的推理配置来源，不保证覆盖后续模型 capability clamp 或 provider 特殊归一化后的每一个取值。

## 验证

- 新增跨日志/转换一致性测试，覆盖 low、max、adaptive 默认值、null、disabled、legacy budget，以及两种显式 OpenAI reasoning 控制的优先级。
- 修改前测试失败：上游转换为 `low`，日志为 `high`。
- `cargo test --locked -p aisub2api-providers -- --quiet`：405 项通过。
- `cargo clippy --locked --workspace --all-targets -- -D warnings`：通过。
- `cargo fmt --all --check`、`git diff --check`、`cargo build --locked -p aisub2api-cli`：通过。
- 最终构建重跑上表四组真实客户端场景，逐项断言上游 effort、最新请求日志、完成标记和目标 MCP 调用。

## 本地证据

临时目录 `/tmp/aisub2api-rea-20261105`：

- `probe_openai.py`：本次隔离实验脚本。
- `openai-probe-before.jsonl`、`openai-effort-log-before.txt`、`openai-effort-wire-before.json`：修改前对照。
- `openai-probe-after.jsonl`：最终构建的四组运行摘要。
- `captures-openai/<case>/client-requests.json`、`requests.json`：转换前后请求。
- `captures-openai/<case>/proxy.log`：请求配置日志；同文件可能含多次实验，以对应运行的最后请求记录核对。

临时请求和第三方模块内容不加入版本控制。真实长会话压缩、流式断连恢复、模型端成本与质量仍需单独验证，本轮不将它们计为已优化。
