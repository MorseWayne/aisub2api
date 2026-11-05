# 社区实现与反馈的兼容性审查

日期：2026-11-05。范围：结合同类项目的公开 issue、修复 PR、代码 diff，与 aisub2api 源码和本地实验交叉验证。社区报告描述其特定版本，不直接作为当前其他项目仍存在缺陷的结论。

## 优化依据

优先从本项目真实用户故障、生产请求和可复现的同类社区反馈确定问题，再用官方契约、客户端实现与同类修复代码确认边界。代码中已有某个功能，或一个项目很流行，都不自动构成新增需求。

每条线索记录来源、版本、触发条件、issue/PR 状态、影响的请求路径和本地验证结果。Closed 不等于修复已合并，PR 作者声明和自动 review 结论也需要回到 diff、实际调用路径和测试验证。

处理结果分为：

- **已覆盖**：保留对应回归，不为已有正确行为另加实现。
- **已复现需修复**：先构造失败用例，再做范围匹配的修复和验证。
- **待验证**：缺少真实调用路径、输入样本或上游权限时，记录重新评估条件，不按猜测新增状态或 capability。

客户端逆向用于解释发送条件和恢复分支；社区反馈用于补充真实使用场景；官方契约用于确定协议要求；最终由本项目的源码和复现结果决定变更。

## 第一批证据对照

| 来源与版本 | 社区问题/实现 | 本项目判断 | 处理 |
| --- | --- | --- | --- |
| [LiteLLM #36540](https://github.com/BerriAI/litellm/issues/36540)，报告 LiteLLM 1.95.0 / Claude Code 2.1.220 | tool_result 中含 tool_reference 时被整个删除，导致 Responses function_call 缺少 output | 当前 `tool_result_output` 对未知 typed block 保留文本表示；转换后 call/output identity 仍应配对 | 新增合成回归，验证混合 text/reference 的结果、匹配 call_id 和发现后的 schema；通过，未新增生产逻辑 |
| [LiteLLM PR #36557 的 diff](https://github.com/BerriAI/litellm/pull/36557/files) | 当前查看的改动保留 text/tool_reference，把引用表示为 loaded-tool 文本，并测试直接 Responses 路径 | 可借鉴“保留结果及调用关联”这个约束；不要求本项目采用完全相同的文本格式 | 已读取实际转换 diff；PR 当前仍 Open，不将其写成已合并修复 |
| [claude-code-router #1397](https://github.com/musistudio/claude-code-router/issues/1397)，报告 v2.0.0，含后续用户复现 | reasoning→tool_calls 转换可能损坏参数；后续反馈还提到 text delta 落入 tool_use 索引 | 当前转换器分别维护 thinking/text/tool block 索引，需要验证边界组合 | 新增合成流回归：reasoning→分段工具参数→尾随换行→tool_calls finish；检查 delta 类型、生命周期和参数 JSON；通过 |
| [claude-code-router #1917](https://github.com/musistudio/claude-code-router/issues/1917) / [PR #1918](https://github.com/musistudio/claude-code-router/pull/1918)，2026-10-25，当前均 Open | 上游 headers 已发后断流，调用方只收到 EOF；PR 提议明确的协议错误及完整事件边界处理 | 与本项目此前已复现的原生 Messages 断流缺口属于相同问题类别，不能当成完全相同根因 | 对照 v6.0.4 的流式完成边界修复；已有文本/thinking 截断、终结后关闭、尾帧和真实客户端验证。本次未读取到该 PR 的完整文件 diff，不宣称采用其实现 |
| [CLIProxyAPI #1894](https://github.com/router-for-me/CLIProxyAPI/issues/1894) / [PR #1892](https://github.com/router-for-me/CLIProxyAPI/pull/1892)，2026-03 | 讨论显式过滤未加载 schema，并将 tool_reference 转成 schema 文本 | 该报告对应较早的发送形状。当前固定 2.1.318 实验初始只有 placeholder/ToolSearch，发现后客户端自己加入对应 schema | 保留版本差异；PR 为 Closed、未显示合并，不按“Closed 即成熟方案”复制整个工具加载机制 |
| [copilot-api #339](https://github.com/caozhiyuan/copilot-api/issues/339)，2026-07，当前 Closed | 加密函数输出无法解码；维护者提及 Codex/Copilot encrypted content 不兼容，报告还经过 new-api 层 | 不能仅凭外形校验证明 encrypted blob 可跨 provider/账号复用；多层代理使根因仍需更细的样本 | 待验证。需明确签发方、账号/provider 作用域与重放流程；本轮不增加签名来源缓存或改写 opaque 内容 |

本仓库公开 issues 页面本次未能通过读取工具获取有效内容，不能据此推断没有用户反馈。

## 实现比较中的边界

LiteLLM 的报告还强调：它观察到的客户端会把已发现工具的 schema 放进 tools，因此保留引用结果及 call/output 配对即可恢复该路径。CLIProxyAPI 的旧提案则包含更完整的服务端 schema 注入。二者覆盖的版本、入口和已有行为不同；不应拼成一个默认开启的代理状态机。

当前本项目受控 MCP 实验已确认 2.1.318 的发送形状，但模型回复由 fixture 驱动，不能据此证明真实模型的理解能力、缓存收益或最优 token 成本。对这类性能结论，仍需在明确上游与预算下测量。

## 本次验证与改动

新增测试：

- `responses::tests::community_tool_reference_results_preserve_call_output_pairing`。
- `chat_completions::tests::community_reasoning_tool_call_and_trailing_text_keep_block_types_and_arguments`。

两项使用独立构造的合成输入，没有复制用户日志或凭据。测试以社区故障的协议不变量为断言，不要求本项目实现与社区项目一致。

`cargo test --locked -p aisub2api-providers community_ -- --nocapture`：2 项通过。Clippy、格式和 diff 检查按本次改动范围执行。本次社区对照没有新增生产逻辑；回归与记录和 effort、流式修复一并纳入 v6.0.4。

## 后续优先级

1. 将本项目真实错误样本与用户反馈放在最高优先级，关联具体客户端版本、provider、接口及失败阶段。
2. 工具调用和恢复：从工具结果配对、分段参数、thinking/tool/text 切换、断流及非 2xx 错误语义继续建立场景矩阵。
3. 长会话与跨 provider：有真实样本后审查缓存标记、压缩前后身份及 encrypted 内容归属，不先增加代理侧隐式状态。
4. 每项优化同时记录正向效果和代价；涉及 token、延迟或吞吐时使用实际调用测量，不把兼容性测试通过当作性能收益。
