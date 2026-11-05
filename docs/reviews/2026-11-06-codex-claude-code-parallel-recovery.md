# Codex 转 Claude Code 的并行工具与恢复验证

日期：2026-11-06，Asia/Shanghai。验证代码基线：`e28aac5f098514c12c04a837c6d51edbd53866de`；记录验证阶段的工作区补丁，后续纳入 v6.0.6。实际发布状态以 tag 和 GitHub Release 为准。

结论：保留 Lite 对 `parallel_tool_calls=false` 的约束；对支持 Full 的模型逐项选择 Full，并补齐 Messages SSE 在输出前的有界 overload 恢复。配置按 provider 和 upstream model id 生效，不能把 Astra 的结论推广成所有 CC 模型都关闭 Lite。

## 当前模型和配置边界

只读检查现有别名，得到四个不同的 upstream model。模型名是策略的 identity；`sonnet`、`opus` 等只是客户端路由别名，不代表上游能力。

| 当前别名 | Upstream model | 配置草案 | 本机实测证据 |
| --- | --- | --- | --- |
| default、sonnet | gpt-5.6-sol | Full | SSE、Auto→WS 短流程；SSE 和 WS 的默认提示、约 26k 历史、MCP/Read 和回忆流程通过 |
| reasoning | gpt-6.1-sol | Full | SSE、Auto→WS 短流程；SSE 和 WS 的约 26k 历史通过；WS low/high effort 均验证实际参数 |
| opus | gpt-6-astra | Full | 多次首轮、Auto→WS 短流程、ToolSearch/MCP/Read、多轮历史及缓存对照；保留容量拒绝记录 |
| haiku | gpt-6-luna | Lite | SSE、Auto→WS 均串行完成两个 Read 并给出正确 JSON，各 3 次模型请求 |

[配置片段](2026-11-06-claude-code-multi-model.toml)使用现有 `model_capabilities`，不新增配置字段或生产代码模型白名单。全局必须保持 `responses_lite="auto"`：`on`、`off` 的优先级高于逐模型 override。未列出的模型继续走原有 Auto 判定；Auto 本身不保证未知模型采用 Lite。

该片段只更新 lane 策略，不替换完整配置，不改现有别名及其 reasoning effort、proxy、transport 或 runtime。配置 key 使用不带 `chatgpt/` 的 upstream model id。

**影响面：同一 provider 下的 Messages 和原生 Responses 都会读取这份策略。** 已覆盖四个模型的 Auto→WS 短流程、三个 Full 模型的 SSE 多轮历史，以及两个 Sol 的长历史 Auto→WS 组合；另覆盖 gpt-6.1-sol 的 high effort 参数路径。本轮透明 overload 恢复只作用于 SSE，不能把 WS 成功验证描述成同样具备输出前 overload 恢复。

本轮不推荐复制一个 `chatgpt-cc` provider 来隔离策略。源码中每次 `ChatGptProvider::new` 都新建 `ChatGptAuth`；token 路径固定为配置目录下的 `chatgpt/token.json`，刷新锁和内存 token 却属于各自实例。由此推断，同进程多个别名可能竞争同一刷新 token，不能称为账号隔离或已验证的安全隔离。只有确认需要原生/CC 分别控制 lane，且完成共享登录态与刷新同步验证后，才重新评估这一方案。

## Lite 和 Full 的协议证据

最初的受控 A/B 保持 Lite header、`reasoning.context=all_turns` 和业务请求不变，只改 parallel：

| 条件 | 样本 | 上游结果 |
| --- | --- | --- |
| Lite + parallel=false | 3 次 | 3 次 HTTP 200，每次一个工具调用 |
| Lite + parallel=true | 3 次 | 3 次 HTTP 400，`unsupported_value`，param 为 `parallel_tool_calls` |

上游错误明确要求 Lite 的 parallel 为 false。目录同时报告 `supports_parallel_tool_calls=true` 和 `use_responses_lite=true`，说明独立能力字段不能覆盖当前 lane 的协议限制。

随后 Astra 三组各三次的对照：Lite/false、Full/false 都每次返回一个调用；Full/true 每次返回两个，全部 HTTP 200 并完成。完整工具流程中 Full 可减少模型往返，但也出现 HTTP 200 内嵌 overload，不能仅用首轮成功宣称完成稳定性验证。

社区也采用逐模型选择：[Raine 的 Codex 文档](https://claude-code-proxy.raine.dev/providers/codex/#responses-lanes-and-parallel-tools)允许一组明确模型 opt-in Full，Luna 仍保持 Lite。这里对 Luna 的部署选择结合社区实现和本机 Lite 成功路径；本轮没有通过请求 Luna Full 来证明它在所有账号上都不可用。

## 社区实现的可借鉴边界

下表来自固定源码快照的路径检查，不代表执行过这些项目。详细快照和选取文件在 `/tmp/codex-proxy-community-20261105`。

| 项目及快照 | Lane 和并行策略 | 恢复边界及采用结论 |
| --- | --- | --- |
| [CLIProxyAPI](https://github.com/router-for-me/CLIProxyAPI/tree/585f97deac9aa57c700301221fb1a7085e1827cc) | CC 普通 Responses 默认 parallel=true；显式 Lite 标记时强制 false | 有启动缓冲和 overload failover；该缓冲默认关闭，不将“源码存在”写成默认生效 |
| [CC Switch](https://github.com/farion1231/cc-switch/tree/f621d4216189ef06bcc6acd3588c84937b9794e1) | 普通 Responses 默认并行，保留客户端禁止并行的语义 | 首部事件验证与缓冲后再转发；[PR 7024](https://github.com/farion1231/cc-switch/pull/7024)已合并并进入 3.20.2 |
| [sub2api](https://github.com/Wei-Shaw/sub2api/tree/25608d216bb7be04e4a646830cb065cd09b7d7e9) | CC bridge 普通 Responses 默认并行；专用 Lite 路径强制 false | native 路径有语义输出前恢复；CC 转换器会在 response.created 提早发 message_start，不能推断两条路径恢复完全相同 |
| [Raine](https://github.com/raine/claude-code-proxy/tree/49cf15220b2efa94b3d21c1a560d3762e961628d) | 明确模型 opt-in Full，Luna 保留 Lite | 保留初始化事件、共享有界请求预算；Full 改动在该 main 快照，但早于它的 0.1.47 release 不包含此改动 |
| [codex-as-api](https://github.com/Eunho-J/codex-as-api/tree/7ac16dde64e83b0bc3c1ec638c2f6cd0af01a854) | 未显式传 parallel 时默认 false；Lite+true 拒绝 | 关闭 Lite 本身不等于自动启用并行 |
| [LiteLLM](https://github.com/BerriAI/litellm/tree/aa7917a3d2634c420faed46d4b2aaf87054b8728) | ChatGPT Responses allowed_keys 过滤 parallel 字段 | 不能视为具备完整的显式 parallel 控制方案 |
| [claude-code-router](https://github.com/musistudio/claude-code-router/tree/057abaa1f95519ab8faf999e1ccef2b80ccdeb9e) | 委托锁定的 gateway 1.0.21；普通 Responses 规范化 | 选取源码和校验过的 npm 发布产物未见专用 Lite/parallel 策略，不推断存在完整解法 |
| [new-api](https://github.com/QuantumNous/new-api/tree/72f3dc7daaeba86280b01e2d4d5d745df5c7c67b) | Messages→Responses 需显式启用路由策略，默认关闭；未禁止并行时省略该字段 | 先确认转换策略实际启用，不能只检查 Codex adapter |

共同可采用的是：区分 lane 契约和模型能力；在对客户端暴露输出前决定重试；共享预算；输出后交回客户端恢复。无需照搬额外工具注入、账号 failover 或服务器上下文压缩。

## 本轮补丁

- `chatgpt/sse_recovery.rs`：Messages SSE 暂存已知的空初始化事件，从第一个成功 HTTP 响应头开始，暂存窗口最多 5 秒；累计达到 64 个事件或 256 KiB 时交付。单个事件仍受现有 SSE frame 限制。它不是整个请求的总超时。
- 显式 overload 且尚未交付输出时，才在已有 HTTP retry allowance 内重试。HTTP 和 SSE 不各自扩大一份预算；保留原有一次 401 凭据更新语义。请求业务 body 保持一致。
- 文本、thinking、工具调用、加密 reasoning、未知事件、成功终态或暂存达到边界后交付流，之后不透明重放。取消请求会停止读取、等待和后续尝试。
- SSE 事件优先使用自身 retry advice，缺失时补 HTTP header；长于 5 秒的建议交回调用方，不能提早重试。policy capacity 不进入这里的透明 overload 重试。
- Anthropic 流式错误保留 `overloaded_error`、`rate_limit_error` 等类型；共享请求的 leader 和 follower 一致。Chat/原生 Responses 的错误编码不改。
- 保留 `x-oai-request-id`。SSE 错误已有 metadata 但缺少 request_id 时，补入 HTTP 的 ID；事件自身已提供 ID 时保留事件值。真实形态的 `turn_rate_limit_exceeded` HTTP 429 保持 429 和 RateLimited，保留 Retry-After 与诊断信息，不凭文案改成 529。

生产逻辑不限定 Astra 或上述四个模型。配置只负责 lane；恢复只负责输出前失败边界。

## 验证结果

固定客户端 Claude Code **2.1.318**，SHA256：`708c77b204db30086ff6f1ed34577bda03e1be74ced80557cbf7b54249ebf0bd`。真实模型验证使用的隔离 debug binary SHA256：`75340ff3897de99675a8120535345b7199ff7a8a24ce1d559ecfcb72def7f386`。随后补充 SSE request ID 回填，重建候选 binary 为 `f935ba8d335410cc2b4a8dc31728e4e43589a0cd7d5db5f83542fb1098fc16c0`；该差异通过下述本地 HTTP 冒烟和自动回归验证，没有将此前真实模型样本冒记为新 binary 的调用。

### 协议和真实客户端恢复

本地 HTTP 夹具验证 503、两种 SSE overload envelope、预算耗尽、工具后失败以及 Retry-After。真实 CC 的 7 个协议场景验证输出前恢复、529 后客户端重试、部分文本后恢复和工具结果保留。工具已经执行后，客户端带原结果继续，代理不重复执行工具。

该 CC 版本也会重试旧的 `api_error` 控制组，因此这里只确认错误类型更准确，**没有证据证明 typed error 本身提高成功率**。独立 Retry-After=152 秒的 429 夹具验证未提早重试，且保留上游请求 ID。

### Astra 短任务和容量拒绝

最初真实 CC 三对 A/B：Lite 3/3 成功，Full 2/3 成功；Full 第三次收到 `turn_rate_limit_exceeded` HTTP 429。独立等待超过十分钟后再跑一对，均成功，保留为恢复样本，不替换失败。合计 8 次流程、7 次成功、19 次上游请求。

原始失败没有保留 Retry-After，不能用后补的 152 秒夹具解释该次 CC 为什么退出。该错误也不能直接证明订阅余额耗尽，或证明 Full 导致容量拒绝。

### Astra 默认系统提示、多轮历史和缓存

非 bare、restricted 的真实 CC，每个模式使用一个持久进程，3 个用户回合：保存合成历史及早期 marker；ToolSearch 发现两个只读 MCP 工具并读取文件；无工具回忆前述值。两个历史规模各一对，共 4 个流程、24 次上游请求，全部完成。

| 首轮输入规模 | Lite 模型请求 / 总耗时 | Full 模型请求 / 总耗时 | 最后一轮 Lite / Full cache read 比例 |
| --- | --- | --- | --- |
| 约 8k tokens | 7 / 27.94 秒 | 5 / 20.92 秒 | 97.83% / 97.22% |
| 约 31k tokens | 7 / 37.472 秒 | 5 / 26.665 秒 | 99.36% / 99.59% |

每个流程中 MCP 实际执行各一次，Read 一次，call/result ID 匹配，早期 marker 保留。Full 在同一原生响应返回三个独立读取。每个前端响应的 `cache_read_input_tokens` 与原生缓存计数一致。ToolSearch 加载 schema 会改变前缀，早期未命中不能据此判定 Full 没有缓存。

每个规模仅一对，不构成稳定性能收益的统计结论。累计非缓存 input 接近，不能由累计 input 下降推断实际账单或订阅配额节省。31k 没有覆盖接近上下文上限或自动压缩。

### 本轮新增多模型验证

三个单次 `/v1/messages` 首轮探针，分别使用实际别名 sonnet、reasoning、haiku，全部 HTTP 200；日志确认解析为表中的 model 和 lane。两个 Sol 各返回两个工具，Luna 返回一个。探针不执行虚拟工具。

随后三个真实 CC 工作流，仅开放 Read，读取 `alpha.txt`、`beta.txt`，按真实结果返回 `{"alpha":17,"beta":25,"sum":42}`：

| 模型和 lane | CC 流程结果 | Read / result 数 | 上游请求数 | 耗时 |
| --- | --- | --- | --- | --- |
| gpt-5.6-sol Full | success | 2 / 2 | 2 | 6.306 秒 |
| gpt-6.1-sol Full | success | 2 / 2 | 2 | 6.956 秒 |
| gpt-6-luna Lite | success | 2 / 2 | 3 | 8.298 秒 |

三组工具 ID 全部匹配，两个文件各读一次，最终 JSON 正确。方法为 bare、固定 system prompt、SSE、low effort，每个模型一次，不用于比较不同模型速度。后续默认提示和长历史验证单列如下。

### 四个模型的 Auto 和真实 WebSocket

每个模型新建隔离 provider 实例，避免 provider 级的 sticky fallback 污染后续模型。固定 CC、bare、同样的两个 Read；配置 transport=auto，三个 Full、Luna Lite。

| 模型 | CC 结果 | 实际 WS 模型请求 | 复用连接 / 使用 continuation 的请求 | SSE 降级 |
| --- | --- | --- | --- | --- |
| gpt-5.6-sol | success，两个文件各读一次 | 2 | 1 / 1 | 0 |
| gpt-6.1-sol | success，两个文件各读一次 | 2 | 1 / 1 | 0 |
| gpt-6-astra | success，两个文件各读一次 | 2 | 1 / 1 | 0 |
| gpt-6-luna | success，两个文件各读一次 | 3 | 2 / 2 | 0 |

共 4 个流程、9 次真实 WS 请求。INFO 日志中每次实际选择 websocket，每次 terminal 的 status 都是 completed；请求数与终态数一致，没有 HTTP/SSE 请求。最终 JSON、工具/result ID 和工具执行次数全部正确。这里观察的是原有 WS 成功与 continuation 路径，没有注入真实上游故障。

另增本地回归，覆盖 Auto 遇到 WS HTTP 503 后降到 SSE，SSE 初始化后再收到两种 overload envelope，随后在共享 SSE/HTTP 预算内恢复。断言只暴露胜出响应的一组 message_start/message_stop、原业务 body 一致，且同 provider 下一请求保持 SSE，不再尝试 WS。WS 握手尝试不计入本轮新增的 SSE/HTTP allowance；这是原有 Auto 降级边界，不宣称 WS 与 SSE 共享总次数。

### 两个 Sol 的默认提示和多轮长历史

两个模型各使用一个持久、非 bare、restricted 的真实 CC 进程，默认 system prompt、SSE Full、low effort。三个用户回合：保存合成档案及早期 anchor；ToolSearch 发现两个 deferred MCP 工具，分别读取值，并 Read 一个文件；无工具回忆。

| 模型 | 首轮 input tokens | 模型请求数 | 最后一轮 native cached / input | 最后缓存比例 |
| --- | --- | --- | --- | --- |
| gpt-5.6-sol | 25,596 | 5 | 25,984 / 26,144 | 99.39% |
| gpt-6.1-sol | 25,596 | 5 | 25,856 / 26,051 | 99.25% |

共 2 个流程、10 次真实上游请求，全部 HTTP 200 + response.completed，前端各对应一组 message_stop。两个流程均先完成 ToolSearch，再在同一原生响应返回两个 MCP 调用和 Read；三个读取实际各执行一次。两个结果回合的 anchor、三个值和 total 均正确。

逐请求验证完整档案起止 marker、tool call/output ID、稳定的 system hash 和 prompt cache key；前端 cache_read_input_tokens 与每个原生响应的 cached_tokens 完全一致。请求 relay 保持原始压缩字节，未改 JSON。每个模型只测一次 Full，没有重跑 Lite 对照；缓存比例不推出账单或配额节省，约 26k 历史也没有覆盖 context 上限或自动压缩。

ToolSearch 显式设置 `ENABLE_TOOL_SEARCH=true`，并以实际 defer_loading、ToolSearch 调用、加载后 schema 和 MCP 执行确认生效；仅设置变量不作为验收。[Claude Code 官方环境变量文档](https://code.claude.com/docs/en/env-vars)描述了该开关和 MCP 连接等待设置。

### 长历史 Auto 和 WS 的组合及 high effort

沿用默认系统提示、约 26k 合成历史、三个用户回合和同样的 MCP/Read 任务；新增 WS relay，对 JSON 消息 payload 原文透传，记录实际上游参数及原生终态。relay 先通过本机夹具：请求与响应原文一致、前奏事件透传、同连接 previous_response_id 引用解析和 call/output 配对均通过。此观测增加一跳，不用于速度对比。

| 模型 | 实际 effort | 首轮 input | WS 请求 | continuation 请求 | 最后一轮 cached / input |
| --- | --- | --- | --- | --- | --- |
| gpt-5.6-sol | low | 25,598 | 5 | 3 | 26,240 / 26,486，99.07% |
| gpt-6.1-sol | low | 25,599 | 5 | 3 | 25,856 / 26,071，99.18% |
| gpt-6.1-sol | high | 25,599 | 5 | 3 | 25,856 / 26,057，99.23% |

3 个流程、15 次真实 WS 请求全部 response.completed，零 SSE 降级。每个流程只建立一条真实上游 WS 连接；后续四次请求复用该连接，其中三次使用 continuation。每次实际发送的 reasoning.effort 与表中一致。模型均先 ToolSearch，再在一次响应返回两个 MCP 调用和 Read；实际读取各一次、工具/result ID 匹配，最后回合无工具回忆正确。所有前端响应的缓存计数与对应原生 usage 一致。

加载 MCP schema 后，已有 canonical body 变化，代理记录 body_mismatch，回发完整历史；其后恢复 continuation。它是预期的历史回传边界，不是失败或 SSE 降级。relay 依据同连接上已观察到的 response ID、输入及原生输出核对引用上下文，因此后续增量 payload 不含档案 marker 不等于档案丢失。该引用视图的原生 item 数也不能直接等同于代理规范化后的 CC input 数；代理的 assistant prefix 会跳过原生 reasoning item。

gpt-5.6-sol 的 low 流程观测到累计 151 reasoning tokens；gpt-6.1-sol 的 high 流程该计数为 0，故 high 样本证明参数接受、工具和上下文路径正确，不用于声称覆盖高强度推理或比较推理质量。每个条件只有一个流程，不构成可靠性或缓存收益的统计保证。

### SSE request ID 回填的复现和修复

新增回归先复现：SSE error envelope 已创建 metadata 时，HTTP header 的 x-oai-request-id 没有补入 metadata.request_id，得到 None。server 的 x-upstream-request-id 读取这一字段，保留 metadata.headers 本身不足以把 ID 暴露给客户端。

修复只在缺失时回填 HTTP request_id，保留事件 ID 的优先级。重建 binary 后，用真实 `/v1/messages` handler 接本地虚构 OAuth/SSE 上游，分别验证 error、response.failed，两种 envelope 再分别覆盖无事件 ID、有事件 ID，共 4 例。全部返回 HTTP 529、overloaded_error、Retry-After=9，并正确返回 HTTP 或事件对应的 x-upstream-request-id；每例只有一次上游请求，没有提早重试。

### 自动检查

- `cargo test --quiet -p aisub2api-providers -p aisub2api-server`：425 provider + 118 server unit + 23 integration，共 **566** 项通过。本地 TCP/WS 夹具需要允许监听；沙箱内禁止监听的失败不作为产品失败。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo build -p aisub2api-cli`：通过；重建候选 binary 的 4 个 HTTP request ID 冒烟场景通过。
- `cargo fmt --all --check`、`git diff --check`：通过。
- 新增回归直接解析交付的 TOML，检查四个模型的 lane、并行默认值、显式禁止并行、原生 Responses 影响及未列出模型的 Auto 路径。

## 迁移和回滚草案

验证阶段交付补丁、配置片段和证据时，未修改 live 配置、未替换 binary、未重启 8082。其后用户明确授权提交、发布和本地部署；以下保留迁移及回滚边界。

已完成当前补丁的源码复核，覆盖 HTTP/SSE 共用次数、一次凭据刷新、输出提交边界、取消、Auto 降级入口、native Responses 调用方及共享请求的错误编码，未发现新的阻断项。沿用已通过的 566 项测试、Clippy、构建和候选 binary 冒烟结果；本次配置差异准备未改生产代码，不重复发送真实模型请求。

[实际配置 diff](2026-11-06-claude-code-config.diff)依据当前 live 配置快照生成，只有四个新增的模型 table；全局 Auto 已存在，其他配置逐项比较保持一致。该 diff 已在私有临时副本上应用，并使用最终候选 binary 执行 `config validate` 通过。临时完整配置副本及日志已删除，仓库只保存不含凭据的差异。diff 是当前快照的零上下文补丁；真正启用前应重新核对配置，不能假设行号或既有 override 永远不变。

当前范围的补丁、配置、恢复回归和受控客户端证据已齐，可进入代码与配置评审。后续启用顺序：先部署恢复补丁，再逐模型启用 Full。每次确认实际 CC alias→model、lane、所选 transport、工具/result 唯一配对、成功终态及缓存计数；保留 429 和服务端 Retry-After。自动压缩、context 上限、其他模型及 effort 不属于当前验收样本，仅在实际使用或故障涉及这些边界时扩展验证。

如需落到共享 provider 的 live 配置，先确认接受对原生 Responses 的同名模型影响；否则继续保留原策略，先解决共享认证的隔离问题。不要直接追加重复 TOML table，也不要用该片段覆盖完整配置。

配置回滚单位为这四个模型的 `responses_lite` override：恢复启用前的值；原来不存在则只移除新增字段，保留其他 capability。全局恢复原值。已有活跃会话不在回滚中清空。代码回滚单位为本轮恢复补丁及错误编码，保留其他用户改动；实际服务操作另行授权。

## 本机证据位置

以下为合成数据和脱敏记录；临时 token、配置、原始日志所在私有 runtime 目录已经清理。原 token 副本禁止刷新，原始凭据及 live 配置 hash、8082 PID 均核对未变。

- `/tmp/codex-ab-20261105/evidence`：Lite parallel A/B。
- `/tmp/codex-lite-off-20261105/evidence`：三组首轮 lane/parallel 对照。
- `/tmp/codex-tool-flow-20261105/evidence`：完整工具流程及原始 overload 失败。
- `/tmp/aisub2api-recovery-smoke-results.json`：代理 HTTP/SSE 夹具。
- `/tmp/aisub2api-cc-validation-20261106/evidence/validation-summary.json`：真实 CC 协议场景、原始容量失败和独立恢复对。
- `/tmp/aisub2api-cc-history-20261106/evidence/validation-summary.json`：默认提示、多轮历史、MCP、缓存及清理记录。
- `/tmp/aisub2api-cc-multimodel-20261106/summary.json`、`cc-summary.json`：本轮三个模型首轮及真实 CC 验证；模型和 lane 使用 INFO 日志核对，不声称保留了原生 SSE terminal。
- `/tmp/aisub2api-cc-transport-20261106/<model>/cc-summary.json`：四个模型的 Auto→WS、连接复用、continuation 和完成终态。
- `/tmp/aisub2api-cc-sol-history-20261106/<model>/`：两个 Sol 的默认提示、多轮历史、双向安全摘要和逐项验收断言。
- `/tmp/aisub2api-cc-ws-history-20261106/<model>-<effort>/`：长历史 Auto→WS、实际 effort、原生 usage、continuation 引用及双向安全摘要。
- `/tmp/aisub2api-cc-ws-history-20261106/request-id-smoke.json`：最终候选 binary 的四个本地 HTTP request ID 冒烟结果。
