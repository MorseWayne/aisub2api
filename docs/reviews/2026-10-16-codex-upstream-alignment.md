# 2026-10-16 Codex 上游对齐

## 同步与审查范围

- 上游仓库：`https://github.com/openai/codex.git`，分支 `main`，跟踪 `origin/main`。
- 同步前及上次审查基线：`0d46c252b3f29f10bacf0ef58a17a1aa5d17ead3`。
- 本次 fetch 后同步到：`e790ce271749ce431e446a9e2cc66485fe60f931`，`Use a synchronous mutex for remote metadata request tracking (#54475)`。
- 实际审查范围：`0d46c252b..e790ce271`，共 1,820 个提交。通过提交历史、文件差异及最终请求实现筛选网络协议相关变化，不逐个审查 UI、沙箱和本地执行实现。
- 参考仓库执行 `git reset --hard <fetched_sha>`、`git clean -fd`；同步前没有本地改动或未跟踪文件，无本地工作被删除。同步后 HEAD 与 fetch 结果一致且工作区干净。上游在此范围内删除了根 `AGENTS.md`，没有重新添加本地版本。
- claude-proxy：工作从 `c885f6d2063030ed6cbae033656615d756c634b9`、版本 `4.0.2` 开始。本次对齐纳入 `v4.0.3`（2026-10-17）发布。原有 `.cargo/`、`.claude/settings.json` 未修改。

## 契约边界

本次证据来自同步后的 Codex 源码及本地模拟服务测试，不声称已验证真实 ChatGPT 账户权限或私有后端。

代理原生 `/v1/responses` 仍要求 `stream=true`、`store=false`，只路由到 OpenAI/ChatGPT；下游无状态，不开放 WebSocket。ChatGPT Messages 的上游 WebSocket 复用与原生下游 Responses 是不同路径。公共 OpenAI 模型能力与 ChatGPT 目录保持分离；新增 ChatGPT 模型没有据此获得未经核实的公共 OpenAI 模型规格。

## 对齐矩阵

下表上游路径均相对于 `codex-rs/`；提交链接定位已核查的变化，最终行为以 `e790ce271` 为准。

| 上游证据 | 协议变化 | 代理现状及落点 | 状态 | 处理 | 兼容影响 | 验证 |
| --- | --- | --- | --- | --- | --- | --- |
| [49e95cc73](https://github.com/openai/codex/commit/49e95cc73)、[b1e72963c](https://github.com/openai/codex/commit/b1e72963c)，`models-manager/models.json` | 增加 GPT-6 Sol/Luna、GPT-6.1 Sol；Lite、推理档位和上下文元数据 | `CHATGPT_MODEL_SPECS` 原来缺少这些回退项 | 已修复 | 增加三种模型，使用 272K 默认窗口、Lite、图像输入；保留线上目录和配置优先级 | 模型目录不可用时仍能按已知能力构建请求；不修改用户所选默认模型 | 新模型请求构建、工具、推理别名、sampling 剔除测试 |
| [7abf2a3b5](https://github.com/openai/codex/commit/7abf2a3b5)，`protocol/src/openai_models.rs`、`core/src/session/tests.rs` | Flex 可显式使用而无需目录声明；请求层不自动套用目录默认档位 | `codex_service_tier` 原来会因目录缺少 Flex 而丢弃配置 | 已修复 | 保留显式 Flex；无显式配置或 fast mode 时不自动发送 priority | 不将 UI 提示转换成额外的请求档位选择 | 远程目录默认档位忽略、显式 Flex 保留测试 |
| [ae132dc50](https://github.com/openai/codex/commit/ae132dc50)、[78d4d983d](https://github.com/openai/codex/commit/78d4d983d) | 数字自定义 reasoning effort；配置更新由 Codex 模型能力决定 | 原生请求使用 `Value`，不会将数字改成字符串或过滤配置项 | 已对齐 | 保留数字与 `configuration_update`；更新已知新模型的文本档位别名 | Codex 负责决定是否发配置更新，代理保留其选择 | 原生数字 effort 和配置项完整性测试 |
| [6326163b9](https://github.com/openai/codex/commit/6326163b9)、[c9253c497](https://github.com/openai/codex/commit/c9253c497)、[fd75aa116](https://github.com/openai/codex/commit/fd75aa116) | 增量 `additional_tools`、namespace、instructions 转 input 消息 | 原生请求保留顺序和未知项；Messages Lite 已生成 developer 工具/指令前缀 | 已对齐 | 保留多个增量工具项；不对原生输入重新排序 | 标准 Responses 顶层 instructions 仍透传；Codex 的构建方式变化不等于后端禁用旧字段 | HTTP `/v1/responses` 全链路输入等值检查 |
| [7b8b17b97](https://github.com/openai/codex/commit/7b8b17b97)、[8b6bb1c77](https://github.com/openai/codex/commit/8b6bb1c77)、[5ef96ab27](https://github.com/openai/codex/commit/5ef96ab27)、[1b6e0d814](https://github.com/openai/codex/commit/1b6e0d814) | 图片 file_id、partial_answer、输出 ciphertext/status/annotations/logprobs、来源元数据 | 原生项透传及 correlation 恢复不重建这些字段 | 已对齐 | 补充新版输入夹具；不把上传文件端点隐式纳入代理范围 | 保留原生 replay 内容；文件访问权限仍由实际后端决定 | 原生请求完整性及服务端集成测试 |
| [3dc1e2a58](https://github.com/openai/codex/commit/3dc1e2a58)、[1ac689cc7](https://github.com/openai/codex/commit/1ac689cc7)、[9e03960cf](https://github.com/openai/codex/commit/9e03960cf) | 通过流式 Responses 执行远程 V2 compaction，删除旧独立实现，加入后台 compaction | 原生 Responses 已保留 compaction_trigger/context_compaction/compaction | 已对齐 | 沿用原生通路；调度后台压缩属于 Codex 客户端 | 不增加旧 compact 端点 | 原有 V2 compaction 集成测试 |
| [9d8de1967](https://github.com/openai/codex/commit/9d8de1967)、[44dd77b71](https://github.com/openai/codex/commit/44dd77b71)、[f6cf05af1](https://github.com/openai/codex/commit/f6cf05af1) | HTTP、failed SSE、WS 错误携带 Retry-After；避免早于服务端建议时间重试 | HTTP 原来截短至 5 秒；failed SSE 和 WS 连接限制错误原来丢失建议 | 已修复 | 保留秒/毫秒/HTTP-date 建议、请求 ID；限流文本仅作为后备；大于 5 秒时结束本地重试并交还调用方 | 长等待不在代理中持有请求，不提前重试；已开始的流通过事件/日志表达失败 | 90 秒建议不提前重试、91 秒 WS 建议、SSE header 优先级测试 |
| [31ffe2bc9](https://github.com/openai/codex/commit/31ffe2bc9)、[fa8cf4498](https://github.com/openai/codex/commit/fa8cf4498)、[dafb133c5](https://github.com/openai/codex/commit/dafb133c5) | slow_down 为限流；余额/组织/项目额度错误终止；Flex 容量不足单独分类 | 共用 Responses 错误模块，HTTP 与 ChatGPT WS 错误适配，下游错误映射 | 已修复 | 增加额度分类；FlexUnavailable 保留独立语义，开流前返回 503、`x-should-retry:false` | slow_down 从 overload 变为 rate limit；Flex 不自动切换档位 | HTTP、SSE、WS 分类与三种下游协议映射测试 |
| [bc9cad347](https://github.com/openai/codex/commit/bc9cad347)、[816fa702e](https://github.com/openai/codex/commit/816fa702e) | incomplete 是终止状态并保留用量；interrupted 在 Codex agent 内继续执行 | observer 原有用量能力；未知 incomplete 原来转换为正常 end_turn | 已修复 | 原生保持完整终止事件；翻译路径未知/interrupted 理由报错；保留长度截断和内容过滤映射；终止 SSE 立即释放上游 | 代理不模拟 Codex agent 的中断后续轮次；不再把未知不完整结果报告成功 | 保持 TCP 打开时终止、原生事件等值、错误前记录 reasoning/output tokens |
| [12acac2a6](https://github.com/openai/codex/commit/12acac2a6)，`http-client/src/chatgpt_cloudflare_cookies.rs` | HTTP/WSS 共享基础设施 Cookie，包含失败握手响应 | 旧 `RoutingCookieJar` 仅接入 reqwest | 已修复 | 每 provider 的 jar 共享给 WS；仍只允许 `__oailb`，不扩大为上游所有 Cloudflare Cookie | 保留域名、路径、Secure、过期和 provider 隔离；显式 provider Cookie 优先 | HTTP→WSS→HTTP、过期、外部域名、明文 WS、显式值测试；实际路由收益待实测 |
| [537278c65](https://github.com/openai/codex/commit/537278c65)、[2b8429628](https://github.com/openai/codex/commit/2b8429628)、[794371399](https://github.com/openai/codex/commit/794371399) | 所有者变化清理 WS 与模型缓存 | 模型缓存已有身份隔离；WS key 原来只含 account_id | 已修复 | WS 连接和 continuation 加入 token 身份摘要及配置路由头；复用既有稳定所有者摘要逻辑 | 已知所有者正常换 token 可复用；不透明 token 保守隔离 | 同 account 的不同凭证触发第二次握手且发送完整输入；既有 owner/plan/cache 测试 |
| [58e2e8cf3](https://github.com/openai/codex/commit/58e2e8cf3)、[4d23af097](https://github.com/openai/codex/commit/4d23af097)、[2b21129c6](https://github.com/openai/codex/commit/2b21129c6)、[780d7ab09](https://github.com/openai/codex/commit/780d7ab09) | Workspace/Gateway 路由认证、turn metadata patches、历史初始化元数据 | 原生 client_metadata 保留；provider extra_headers 是显式路由配置；下游不能覆盖认证头 | 部分已对齐，其余不适用 | 保留新元数据；不复制 Codex 的工作区发现、Gateway OAuth 登录和托管网络策略 | provider 路由由部署配置负责；不把客户端凭证当成上游认证 | 原生 metadata 测试和原有头过滤/身份缓存测试；私有 Workspace 路由待实测 |
| [7d4c7a076](https://github.com/openai/codex/commit/7d4c7a076)、[888be42a2](https://github.com/openai/codex/commit/888be42a2)、[458f7046a](https://github.com/openai/codex/commit/458f7046a) | API-key Codex 元数据发现、显式 model_catalog_url | 代理 `/v1/models` 是公开模型列表加代理能力，不是 Codex 的 `{models: ModelInfo[]}` 完整目录 | 不适用：完整 Codex 目录服务 | 保持原有公开目录契约；不伪造 Codex 提示模板、Guardian/tool schema 元数据 | 若将 Codex 的显式 model_catalog_url 指向代理，会遇到不同目录格式；没有宣称兼容该可选接口 | 原有模型目录和 capability 测试 |
| [ffc140263](https://github.com/openai/codex/commit/ffc140263)、[12de0e395](https://github.com/openai/codex/commit/12de0e395)、[ed0cc1a4a](https://github.com/openai/codex/commit/ed0cc1a4a)、[eeded5ba1](https://github.com/openai/codex/commit/eeded5ba1) | 分类器 interrupt/drain、steer 续接、JSON 路由字段前置、Guardian 改走 Responses | 代理取消关闭上游；续接按完整输入/请求属性检查；无 Guardian agent runtime | 已对齐或不适用 | 保留已有取消与保守全量回退；JSON 键序是性能优化，无新增必需字段 | 不引入会话控制端点或后台分类器池 | 既有取消、续接前缀/模型/属性改变和 SSE 回退测试 |

## 验证与后续边界

- `cargo fmt --all -- --check`、`git diff --check`：通过。
- `cargo test --locked --workspace --all-targets -- --quiet`：617 项通过（CLI 43、config 30、core 11、provider 403、server 单元 111、集成 19）。
- `cargo clippy --locked --workspace --all-targets -- -D warnings`：通过。
- 收紧测试中的 socket 读取与锁作用域后，重新验证全部 403 项 provider 测试通过。
- 全工作区测试需要本地 socket 与 Cargo 缓存写权限；沙箱拒绝后使用获准的权限运行。测试仅使用回环模拟服务和合成凭证。
- 上游提交引用均通过本地 `git rev-parse --verify` 验证。

本轮对齐审查没有调用真实模型提供者，也没有修改凭证、重启服务或安装二进制。2026-10-17 按用户后续要求将对齐修改纳入 `v4.0.3` 发布；发布不包含本机服务升级。新的 ChatGPT 模型可见性不保证账户授权；GPT-6.1 的真实运行、Flex 可用性、Cookie 性能收益和私有 Workspace 约束仍需专门的真实服务验证。

沿用上一基线的边界：历史 v4.0.1 Structured Outputs 实测结论不等于本轮实测；memory 请求的缺省 session/thread 头与私有后端要求尚未验证。Guardian 执行、Realtime/WebRTC、图像生成/文件上传端点、远程 exec-server、app-server 及本地 UI 不属于代理公开接口。
