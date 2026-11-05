# 升级后观测：保留上游 403，避免失败请求被当作 502 重试

日期：2026-11-05。源码基线：`1597938`（v6.0.5）。本轮读取实际运行记录，结合 Claude Code 请求记录、官方 SDK 和社区契约，修复 HTTP 错误映射；分析与隔离验证阶段没有替换或重启工作实例。

## 运行身份与观察边界

观测时宿主 PID 1299651 运行此前从 `d6d0d9a` 构建并安装的 6.0.4，`/admin/metrics.server_version` 也返回 6.0.4。其映像 SHA-256 为 `7298c3ede449bdbec8a0bfc1be1e7f8277e32dd6b9052e3a318be49c73c19932`，与上一轮实际重启后的记录一致；包含耗时和版本诊断修复，但不包含本轮新增的 403 修复。发布 v6.0.5 并不会自动替换这个进程。

统一观察窗口为 UTC `[2026-11-05 04:30:00, 06:40:00)`，即北京时间 12:30–14:40。SQLite 使用只读连接、短事务和时间条件，读取后立即关闭；运行日志只读，管理认证凭据不输出。客户端读取当前写入的 JSONL，按时间和请求摘要对照，不输出或提交对话正文。

| 已确认事实 | 固定窗口结果 | 判断边界 |
| --- | --- | --- |
| 完成请求观测 | 284 条，5 条上游错误；277 条 completed、2 条 client_disconnected | 客户端取消不能直接当作代理故障；不混入窗口外的当前累计指标 |
| 会话身份 | 284 条均为真实 stable client conversation identity，synthetic 均为 false | 验证身份解析已生效，不证明所有 continuation 都命中 |
| latency 口径 | 按 provider/model/initiator/秒级完成时间唯一关联 268 对 usage/observability，265 对完全相同，其余仅差 1 ms | 表 id 不具有跨表 identity；16 条有歧义记录不强行配对 |
| 总耗时 | P50 7.133 秒、P95 22.998 秒 | 与旧窗口工作负载不同，不能据此比较版本性能 |
| 首个事件 | 279 个有事件请求，P50 2.060 秒、P95 5.997 秒 | 数据库字段为 first_event_ms；不等于首个可见文本 token |
| token 缓存 | 缓存读占输入相关 token 约 96.99% | 不是逐请求命中率，也不是升级的因果效果或计费节省比例 |
| transport | WebSocket 15 条，其中 14 条复用连接、2 条使用 continuation；SSE 269 条 | 仅反映本窗口，不能以 2/284 作为 WebSocket continuation 命中率 |

运行日志记录 UTC 04:30:53 的 WebSocket first-event 读取失败，随后实例内退回 SSE；附近还有出站代理 connection refused。现有单向 fallback 可以解释后续长期 SSE，不据此增加自动探测、账号熔断或另一套 transport 状态机。

## 实际故障：403 变为 502

UTC 06:32:11–06:32:23 的五个失败请求均为 ChatGPT provider，实际 HTTP 状态为 403。错误体是 HTML 网页，文字说明站点暂时无法加载，并建议稍后重试或检查 VPN；没有读取到 model_not_found、限流或容量不足等结构化错误码。日志同时确认这五次下游响应均为 502。

同一时间段的 Claude Code 2.1.318 JSONL 记录五条 api-request，requestDigest 完全相同，对应请求开始时间为 UTC 06:32:09.243、06:32:11.901、06:32:13.948、06:32:16.981、06:32:22.079。五条代理请求的模型、53 条 messages、24 个 tool results 和 479,413 字节逻辑请求体也一致。客户端开始时间与代理结束时间减总耗时相符，但不存在共享端到端 request ID，不能将这种关联泛化成新的 identity 契约。

源码路径为 `ChatGptProvider` → `UpstreamError { status: 403 }` → `handle_provider_error`。Messages 的 `upstream_error_to_response` 和 OpenAI 的 `provider_error_response` 都缺少 403 分支，落入通用 502。官方 SDK 默认重试 5xx，因此现有映射把原本的拒绝响应放大成重复请求。真实客户端隔离实验进一步验证了这个因果关系。

HTML 403 的准确网络根因仍未定位，不能凭网页文案认定账号无权限、token 失效、特定代理故障或模型容量不足。本次修复恢复客户端可见的 HTTP 语义，不能解除上游网页拦截。

## 官方与社区交叉验证

| 证据 | 核验结果 | 本项目动作 |
| --- | --- | --- |
| [Claude API errors](https://platform.claude.com/docs/en/api/errors)，2026-11-05 查阅 | 403 对应 permission_error；5xx 与 403 的客户端处理语义不同 | 保留未经 provider 特化分类的上游 403，不改成 502 |
| [Anthropic TypeScript SDK client.ts，561a13e](https://github.com/anthropics/anthropic-sdk-typescript/blob/561a13ee02cc9f0645ff7b94c6543cfbd9740031/src/client.ts) | shouldRetry 优先尊重 x-should-retry，默认重试 408/409/429/5xx | 对拒绝请求明确返回 x-should-retry: false，并通过实际客户端验证 |
| [CLIProxyAPI 插件错误契约，dde469d](https://github.com/router-for-me/CLIProxyAPI/blob/dde469d3c2cf7f7ee4dce9b519b0b087b7132dbc/examples/plugin/README.md) | 要求保留上游状态，避免将拒绝错误变成客户端认为可重试的服务器错误；403 使用 permission_error | 采用状态与重试语义约束，不照搬该文档的 insufficient_quota code，HTML 无法证明额度不足 |
| [CLIProxyAPI #5869](https://github.com/router-for-me/CLIProxyAPI/issues/5869)，当前 Closed、标签 Fixed | 反馈 Devin 的 high-demand 容量错误被错误分类后触发长期权限冷却，强调区分具体 provider/code | 属于不同 provider 的反例；本次不增加 403 账号冷却，也不把全部权限类错误强制改成限流。未审查其修复 diff，不宣称采用其实现 |

## 最小实现与影响范围

- `ErrorResponse::permission` 统一构造 permission_error。
- Messages、Chat Completions、Responses 的 HTTP 错误响应保留 403，返回 x-should-retry: false；保留错误消息、x-upstream-status 和上游 request ID 诊断头。
- OpenAI 错误 envelope 明确保留 permission_error，不再把它编码为 server_error。
- 只处理尚未被 provider 特化分类的 UpstreamError 403；已有 model_not_found、限流、容量不足、401、422 和网络错误分支不变。没有改写已经开始的 SSE 流的 HTTP 状态，没有增加重试、缓存或持久化状态。

新增单元回归覆盖 HTML/JSON 两种错误体和三个协议；修改前先观察到断言失败（实际 502，预期 403）。新增 HTTP 集成回归使用 Anthropic provider 验证 Messages/Chat Completions，使用 OpenAI provider 验证原生 Responses，检查状态、错误类型、诊断头和每次请求仅一次上游调用。

## 固定真实客户端复现

Claude Code 2.1.318，二进制 SHA-256：`708c77b204db30086ff6f1ed34577bda03e1be74ced80557cbf7b54249ebf0bd`。使用 bare、禁用工具和 session persistence、显式 settings/MCP 边界；配置、缓存及 Claude 状态均放临时目录，使用假凭据和回环端口的合成 HTML 403 上游，没有调用真实模型。

| 案例 | 代理 HTTP 响应 | Claude 产生的上游调用 | 退出情况 |
| --- | --- | --- | --- |
| 修改前，已安装的 6.0.4 构建 | 502，无明确重试提示 | 45 秒内观察到 7 次，约在 0.4/0.9/2.1/4.3/8.5/17.9/34.3 秒 | 达到本次设定的 45 秒观察上限，被实验终止；不称为无限重试 |
| 修改后，本地 debug 构建 | 403，x-should-retry: false | 1 次 | 约 0.7 秒返回明确 403 错误，进程退出码 1，is_error=true |

这是拒绝请求的失败行为验证，不是成功推理质量或吞吐测试。修复让客户端及时得知拒绝、减少重复发送与等待；不能把请求次数减少直接换算成 token 计费节省。

验证结果：`cargo test --locked --workspace --all-targets --quiet` 共 642 项通过；workspace Clippy（warnings denied）、格式及 diff 检查通过；CLI debug 构建通过。这些结果记录分析与隔离验证阶段，当时现有工作实例继续运行。

本地私有汇总位于 `/tmp/aisub2api-live-20261105-post-upgrade`，客户端复现结果位于 `/tmp/aisub2api-forbidden-before-xsvbe3vv` 和 `/tmp/aisub2api-forbidden-after-qya88_gg`。这些目录不随仓库提交；报告不包含会话正文、工具参数、凭据、实际 IP 或账号余额。
