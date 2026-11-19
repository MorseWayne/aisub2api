# 2026-11-15 Codex 上游同步与适配审查

本轮建议先补齐终止错误的机器码保留和分类，再处理 ChatGPT 上游 WebSocket 大请求的分帧与尺寸协商。推理回放丢弃原因的统计可作为后续诊断改进。新增原生请求字段已可透传，未发现需要补充的内置模型。

## 同步范围

- Codex 参考仓库：`/home/wayne/source/open/codex`，远端 `https://github.com/openai/codex.git`，跟踪 `origin/main`。
- 同步前 HEAD 及上次审查基线：`fbf5f0d3a716d08c254d3e4941d5b12eaff5afe7`。
- 本次 fetch 后的准确目标：`ee344ca9458351a6ea102795fa73110ad3ede5dc`，`Clarify window ID discovery in history tool guidance (#61622)`，提交时间 `2026-11-15T07:55:54Z`。
- 执行 `git reset --hard <fetched_sha>`、`git clean -fd`；同步前工作区干净，没有本地修改或未跟踪文件被替换。同步后 HEAD 等于 `origin/main`，工作区干净，忽略的构建缓存保留。
- 分析范围：`fbf5f0d3a..ee344ca94`，591 个提交、3,045 个变更文件。通过提交历史和路径筛选 Responses、传输、错误、模型目录、认证与原生 item 的变化；UI、生成的 app-server schema 和客户端运行时实现按契约相关性排除。
- aisub2api 审查对象：`bd718581194039c75a03891e66fa64529d4446d7`，版本 `6.0.9`。本轮只更新审查记录和基线，未修改运行时代码或现有部署。

## 对齐矩阵

| 上游证据 | 协议变化 | aisub2api 当前行为 | 分类与最小动作 | 兼容影响及验证 |
| --- | --- | --- | --- | --- |
| [0ed9d6fa4](https://github.com/openai/codex/commit/0ed9d6fa4)、[0811df397](https://github.com/openai/codex/commit/0811df397)、[d9446b4b0](https://github.com/openai/codex/commit/d9446b4b0)、[65f270905](https://github.com/openai/codex/commit/65f270905)、[5b98eaafe](https://github.com/openai/codex/commit/5b98eaafe) | `budget_reached`、`inference_hook_denied`、`invalid_access_program`、`unsupported_access_program`、`content_policy_violation`、`access_program_not_enabled` 为终止错误 | `responses/errors.rs::responses_error_event` 未分类这六个 code；翻译路径输出通用 `api_error`。原生 `response.failed` 保留原码，但原生 `error` 被重编码为 `upstream_stream_error` | **缺口，P1**：保留原生终止错误事件的机器码；在共用错误边界补齐有效状态下的终止分类 | 本地探针已复现。新版 Codex 的 `CodexErrorDetails::retry_policy` 依赖类型决定是否重试；丢码后可能进入通用重试分支，这是源码推断，未运行真实 Codex 客户端重试实验 |
| [bb360e5db](https://github.com/openai/codex/commit/bb360e5db)、[4b345a8de](https://github.com/openai/codex/commit/4b345a8de)、[5b98eaafe](https://github.com/openai/codex/commit/5b98eaafe) | HTTP 403 的 `invalid_workspace_selected`、`codex_workspace_access_denied`、`access_program_not_enabled` 保留独立的不可重试语义 | HTTP 状态保留为 403，且已有 `x-should-retry: false`；但原生 Responses JSON 的 `error.code` 变成 `null`。`/detail/code`、`/error/code` 和身份错误头还需统一考虑 | **缺口，P1**：与上一项一起保留可信上游机器码和既有状态，不仅保留人类可读 message | 三个 HTTP 403 场景均已复现丢码；不能把已有 403 状态与 retry header 等同于完整 Codex 分类兼容 |
| [1885417cd](https://github.com/openai/codex/commit/1885417cd) | HTTP 400 / 无状态流的 `context_length_exceeded` 进入上下文恢复；携带 503 的同名 WS 错误仍可重试 | 原生 HTTP 400 变成无 code 的 `invalid_request_error`；共用流错误函数不检查 status，503 同名事件也变成 `RequestTooLarge` | **缺口，P1**：原生 Responses 保留 400 和 `context_length_exceeded`；流分类增加状态约束，503 保留重试语义 | 400 丢码和 503 误分类均已复现；后者在隔离日志中明确记录为 `request too large`。不可全局改写其他下游既有 413 契约 |
| [d15bf1deb](https://github.com/openai/codex/commit/d15bf1deb)、[bc3ccba6e](https://github.com/openai/codex/commit/bc3ccba6e)、[ed7ca22bb](https://github.com/openai/codex/commit/ed7ca22bb) | 大 WS 消息按 1 MiB 帧发送；握手头 `x-codex-websocket-max-message-bytes` 限制整个消息；首条文本前的 1009 可直接转 HTTP | `chatgpt/transport.rs::send_websocket_request` 仍发送一个完整 Text 帧；握手只使用响应 cookie，未保存消息尺寸上限。首事件前关闭已允许 Auto fallback，但没有专门尺寸协商 | **部分已覆盖，P2**：增加分帧和握手上限检查，复用既有 fallback，不新增恢复状态机 | 静态确认；分帧不会绕过消息总长上限。收益作用于使用上游 WS 的 Messages/Chat 请求；本工具原生 Responses 的 ChatGPT 路径当前使用 SSE。需补大帧、无效上限、1009 前后边界的回环测试 |
| [64b8d019c](https://github.com/openai/codex/commit/64b8d019c) | 完成事件的 `input_transformations` 可报告 reasoning 被移除及原因 | 原生完成事件保留未知字段，但 `notify_stream_metadata` 不提取这些诊断信息 | **可选，P3**：需要排查推理历史复用时，再增加按有限原因分类的计数 | 不属于请求成功的兼容缺口。缺失、null、非数组报告不应使请求失败；不记录加密 reasoning 正文。未测真实后端是否返回报告 |
| [41cbd5b9b](https://github.com/openai/codex/commit/41cbd5b9b) | `prompt_cache_options.ttl` 支持 `30m` / `12h`，与 cache key 分开 | 原生请求保留该对象；OpenAI 与 ChatGPT 的 Messages 转换器也已转发；WS canonical request 保留该字段，值变化会阻止复用不匹配的 continuation | **已对齐**，无需新增 TTL 配置层 | 回环确认原生 `12h` 对象不变；其余为源码验证。没有据此宣称缓存命中或费用改善 |
| [65a5410f7](https://github.com/openai/codex/commit/65a5410f7) | Codex backend HTTP/WS 请求新增可信宿主提供的 `request_country`；省略与显式 `null` 不同 | 原生 JSON 路径保留字段及 null，不从模型、IP 或会话 identity 推导值 | **已覆盖原生契约**，无需为普通 Messages 请求生成国家字段 | 回环验证显式 null 不被移除。国家码格式由上游客户端归一化；本轮未验证私有 backend 的地域语义 |
| [cbd494409](https://github.com/openai/codex/commit/cbd494409)、[340a18ed7](https://github.com/openai/codex/commit/340a18ed7)、[9018f3a3b](https://github.com/openai/codex/commit/9018f3a3b) | 请求保留已分配 item ID；工具音频可用 `file_id` 引用 | 原生 input 不经过 Messages 反序列化，保留 item ID、音频引用和顺序 | **已覆盖透传**；上传、音频解码和 entitlement 不纳入当前能力声明 | 回环确认带 ID 的 message、音频 file 引用与配置更新项保持不变；不代表音频功能端到端可用 |
| [55897425e](https://github.com/openai/codex/commit/55897425e)，以及目标 revision 的 `core/src/session/step_context.rs:61`、`features/src/lib.rs:1860` | Full Responses 的增量工具布局发生扩展；当前 legacy 开关用于 Bedrock/GCP，默认值已不同于最初提交描述 | 原生支持 `additional_tools` / `configuration_update`，并保留顶层 hosted `web_search`；Messages Lite 使用既有增量工具布局 | **已覆盖当前代理契约**，不复制 Codex 的工具生命周期调度器 | 回环确认混合 input 和顶层 tools 不变。以目标 revision 实际代码为准，不能只依据最初 PR 的“默认关闭”描述 |
| [52ff3d1a3](https://github.com/openai/codex/commit/52ff3d1a3)、[83a23c093](https://github.com/openai/codex/commit/83a23c093)、[44ccf3945](https://github.com/openai/codex/commit/44ccf3945) | Orbit originator 目录分区、thread/subagent catalog 快照、模型自带 Apps 指引 | 代理公开自身 capability schema；provider 账号及路由配置拥有目录 identity；客户端 prompt/template 与 thread 快照由客户端管理 | **不适用客户端目录实现**；模型元数据继续按在线目录与显式 override 解析 | 分析范围内没有 `models.json` 变更，无需新增 fallback 型号或调整已配置默认模型 |
| [079e17a42](https://github.com/openai/codex/commit/079e17a42)、[d4a341bd5](https://github.com/openai/codex/commit/d4a341bd5)、[6242064da](https://github.com/openai/codex/commit/6242064da)、[6222faa3c](https://github.com/openai/codex/commit/6222faa3c) | runtime identity、复用前重认证、SPIFFE JWT-SVID、gateway OAuth discovery | 当前代理使用自身 provider 凭据；未声明 Codex hosted runtime / gateway orchestration 接口 | **不适用当前认证范围**；其中终止错误的机器码保留仍属于前述 P1 | 不把客户端 runtime assertion 或任意认证头自动转交给 provider，也不新增凭据源 |
| [92c1a4dc2](https://github.com/openai/codex/commit/92c1a4dc2)、[c8e4cbcd9](https://github.com/openai/codex/commit/c8e4cbcd9)、[16280e19b](https://github.com/openai/codex/commit/16280e19b) | 客户端 completion 后处理、hook 预算、共享 HTTP response 解压 | 前两项为客户端结算/展示流程；代理已有 zstd 请求压缩，响应压缩需按实际协商另验 | **不适用或按需验证**，不改写 provider usage、不增加 completion 缓冲 | 没有生产失败或测量支持新增压缩策略、重试层或连接池参数 |

## 已复现的错误边界

使用当前 `6.0.9` release 二进制、回环 OpenAI 模拟上游和独立配置端口进行 22 组探测。未使用真实账号或真实模型；现有服务未重启。隔离实例在探测后终止。

| 场景 | 数量 | 观察结果 |
| --- | ---: | --- |
| 六个新增终止 code，分别通过 `error` / `response.failed` 发给 `/v1/messages` | 12 | 下游 SSE 为通用 `api_error`，未识别为对应输入/权限/预算终止类型 |
| `budget_reached` / `inference_hook_denied`，分别通过两种事件发给原生 `/v1/responses` | 4 | `response.failed` 保留 code；`error` 的 code 被替换为 `upstream_stream_error` |
| 三个 workspace/access-program HTTP 403 | 3 | HTTP 403 保留，JSON `error.code=null` |
| HTTP 400 `context_length_exceeded` | 1 | HTTP 400 保留，JSON `error.code=null` |
| 带 `status=503` 的 `context_length_exceeded` 流错误 | 1 | 错误日志为 `request too large`，与 Codex 的状态边界相反 |
| 新原生请求字段与混合 input、顶层 tools | 1 | 请求对象保持不变，收到成功的 `response.completed` |

错误用例的模拟上游在错误后还发送一个完成事件，用于检查错误不会被后续成功掩盖。探针验证了最终输出的错误类型和机器码；未运行真实 Codex 重试循环。

关键输入可直接用于后续回归：

```json
{"type":"error","error":{"code":"budget_reached","message":"synthetic rejection"}}
```

```json
{"type":"error","status":503,"error":{"code":"context_length_exceeded","message":"synthetic rejection"}}
```

原生字段探针包含 `request_country:null`、`prompt_cache_options:{"ttl":"12h"}`、`access_programs`、顶层 `web_search`、`additional_tools`、带 ID 和音频 file 引用的 message、numeric reasoning 配置更新。它只证明代理透传，不证明私有 backend 接受每一种组合。

本次详细临时结果：`/tmp/aisub2api-codex-review-20261115-jpwhno4w/results.json`；合成日志在相邻 `server.log`。临时目录不是永久测试入口；以上输入、分组和结果保留在此报告中，后续实现应加入仓库正式回归。

## 建议跟进顺序

1. **P1：统一错误信息的保留与分类。** 覆盖 HTTP、SSE、WS 的 code/status 组合；原生错误保留机器码与原始失败语义，翻译路径使用既有输入/权限错误表达。补齐六个终止 code、三个 HTTP 403、400 上下文恢复和 503 状态约束。验证错误后不会继续输出成功；不只断言 HTTP 200。
2. **P2：WS 分帧和尺寸协商。** 在现有连接结构中保存握手上限，按真正发送的序列化字节检查消息长度；大消息分帧，超总长直接进入既有 HTTP fallback。验证 UTF-8 跨帧、continuation 最终 payload、缺失/无效上限、首文本前后 1009。保持当前 Auto/强制传输模式的边界。
3. **P3：按需增加 reasoning 回放诊断。** 只提取有限原因计数，区分报告缺失与报告为空。原生事件继续透传；统计失败不影响正常响应。

没有修改运行时代码，因此本轮未重跑整套 Cargo 构建和测试。已完成原始 revision 对比、调用链核对、22 组隔离探测及文档 diff 检查。上述 P1/P2 尚未实现，WS 大请求及推理丢弃报告没有真实后端验证。
