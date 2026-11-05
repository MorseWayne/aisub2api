# Claude Code 2.1.318 协议适配调研

日期：2026-11-05。范围：用 REA 和隔离的真实 Claude Code 验证 aisub2api 的 Messages 协议适配，修复有直接证据的缺口。

本文记录首轮 `2180a9d` 的结果。后续定向读取模块 contents、受控 MCP 工具搜索和 HTTP 422 恢复的验证见[第二轮调研](2026-11-05-claude-code-tool-search-recovery.md)；下文未覆盖项对应首轮范围。

## 结论与证据范围

- REA 已在 `/tmp/aisub2api-rea-20261105` 隔离构建，未注册全局 MCP 或安装全局 skill。
- REA 对本机 Claude Code 原生文件的模块提取失败；本轮没有恢复客户端源码或反编译其字节码。
- 真实 Claude Code 向本地模拟接口运行五组场景，记录七个推理请求；REA 成功检查对应 HAR，并保留记录序号和来源摘要。
- 相同五组场景又经过本次构建的 aisub2api，验证完整的 `Claude Code → aisub2api → 本地模拟上游` 调用链。
- 本轮修复协议头丢失、effort 能力误判、Claude Code 会话身份未识别、`thinking.display` 丢失四项问题。模拟上游用于验证协议行为，不证明真实模型的质量、缓存命中率或服务端功能可用性。

## 固定版本与工具来源

| 对象 | 版本或摘要 |
| --- | --- |
| Claude Code | 本机原生 Linux ELF，版本目录 `2.1.318` |
| Claude Code SHA-256 | `708c77b204db30086ff6f1ed34577bda03e1be74ced80557cbf7b54249ebf0bd` |
| REA 源码 | 官方 `morluto/rea`，commit `71f680d7c1aa7a45ba0325fe87b104744b0c105f` |
| REA 版本说明 | 源码 package version 为 `6.8.0`；这是上述 commit 的构建，不能视为 npm `6.8.0` tarball 的同一份代码 |
| Node | 独立的 `24.22.1`，官方 Linux x64 包，以官方 SHASUMS256 校验 |
| Node 压缩包 SHA-256 | `6c172a9ab6718f7b801fb5a7bd486af2ee5e32758086a04b84fcce2aa77095e6` |
| REA 源码压缩包 SHA-256 | `52851165bc96fb8c29b5f5be6d689be0ef6c3dcf74d5ec5c40f2165f23c03e28` |

依赖使用 REA 自带 lockfile，通过 `npm ci --ignore-scripts --no-audit --no-fund` 安装到临时目录，再运行 `npm run build:cached`。未升级本机 Node、npm 或 Claude Code。

## REA 的实际结果

### 原生模块提取：未成功

执行 `rea inspect-artifact <固定的 Claude Code 文件> --json`，返回：

```text
code: truncated
boundary: capacity
Bun standalone readable entries declare 94283717417 plaintext bytes,
exceeding this graph's 94184610000-byte expansion budget.
```

当前文件为 337,814,032 字节。进一步只读检查 REA 已解析的模块表后，确认拒绝的具体机制：

- REA 已识别 Bun graph，解析出 3,100 个模块；graph 大小为 188,369,220 字节。
- 其中 2,855 个 bytecode 引用的结束位置相同、起点不同，形成大量重叠区间；合并覆盖范围为 106,958,560 字节，逐个引用相加为 275,146,473,460 字节。
- `BunStandaloneArtifactReader.#prepare` 对每个逻辑模块的 contents、sourcemap、bytecode 和 module-info 长度逐项累加。共享的 `artifactExpansionBudgetBytes` 策略明确按每个逻辑引用计算工作量，包括物理区间重叠的情况。
- 本输入的预算为 graph 大小的 500 倍，即 94,184,610,000 字节；累计到模块索引 912 的 bytecode 引用时为 94,283,717,417 字节，与原始报错完全一致。

因此，失败发生在完整模块清单的工作量准入阶段。它是当前整包处理策略遇到大量重叠 bytecode 引用时的限制；若仍按每个逻辑引用分别输出，确实可能产生巨量重复数据，不能仅将其当成内存配置问题或断定预算实现有 bug。本轮没有修改 REA 或放宽边界。

入口模块 `/$bunfs/root/cli` 有 26,410 字节的 contents，并带 `@bun @bytecode` 标记。可继续研究仅提取 contents、以引用表示共享 bytecode 的路径；可读内容的业务逻辑覆盖率尚未验证，字节码本身仍未反编译。

作为替代入口，下载并校验了同版本官方 `@anthropic-ai/claude-code@2.1.318` npm 包；其七个文件包含 native wrapper、安装脚本、类型声明等，没有可读的主程序 JavaScript 入口，因此没有转入静态 JS 调用链分析。

REA 的 [Bun 文档](https://github.com/morluto/rea/blob/71f680d7c1aa7a45ba0325fe87b104744b0c105f/docs/bun-standalone.md) 明确限制了布局支持，并说明提取不包含字节码反编译。这里的结论仅是当前固定输入未能提取。

### 协议样本检查：成功

隔离测试记录了客户端请求和模拟接口实际写出的响应，再整理为 HAR 1.2。HAR 明确标记：时间、耗时和 URL 端口已归一化；响应由本地 fixture 生成。不是对 Anthropic 生产服务的抓包。

执行：

```sh
node scripts/rea.mjs inspect-web-network-capture /tmp/aisub2api-rea-20261105/claude-code-local.har har --json
```

结果：

- `operation = inspect_web_network_capture`，`total_records = 7`，ordinal `0..6`。
- HAR SHA-256：`78c3b63bc9166304ed3683246364e289a1c8ae89b9e0366cfd6e828a2004f5eb`。
- Evidence ID：`ev_d7d9ab748a71c2257541696387ffed16ce20b04eaef720746a1070f1f4abc9b8`。
- REA 将其标为 `confidence = observed`、`authority = historical-reference`，不提供运行归属或生产部署真实性证明。

## 真实客户端验证

客户端使用 `--bare --no-session-persistence --strict-mcp-config`，MCP 配置为空，系统提示和文件均为本地 fixture。仅向随机回环端口提供虚构 API key；配置、日志、数据库均位于临时目录，没有使用已有 OAuth 或真实模型凭据。

| 场景 | 本地接口观察 | 客户端结果 |
| --- | --- | --- |
| 普通流式回复 | `message_start`、text block、`message_delta`、`message_stop` | 退出码 0，收到固定完成标记 |
| 工具调用 | Read 的 input JSON 拆成两个 delta；下一轮带匹配的 `tool_result` | 读取临时文件并完成，共两个请求 |
| thinking | thinking delta、signature delta、text block | 成功消费固定的 thinking/text fixture；未验证真实签名或交互终端视觉效果 |
| 低 effort | `thinking.type = adaptive`，`output_config.effort = low` | 正常完成 |
| context management 降级 | 首次返回 `400`，消息为 `context_management: Extra inputs are not permitted` | 第二次请求移除字段及配套 beta，完成回复 |

经过 aisub2api 时，模拟上游收到配置中的虚构上游 key，而非客户端 key；context management 拒绝消息经代理后仍能触发客户端降级。五组场景的客户端退出码均为 0。

## 适配改动

### 1. 保留 Messages 协议协商头

**已确认事实**：Claude Code 发送多个 `anthropic-beta` 能力值和 `anthropic-version`。原来 server 只读取 1M beta 做内部处理，provider 的 HTTP 请求不带客户端 beta。

**改动**：将 HTTP 协议头与 JSON 请求体分开传递；仅对 Anthropic 和 CustomAnthropic 转发 `anthropic-beta`、`anthropic-version`，保留重复值。上游认证仍由 provider 配置生成。其他协议转换沿用既有路径。

**请求身份约束**：协商头影响上游语义，因此纳入 inflight 去重 identity。相同请求体、不同 beta 或 version 不共享上游响应。没有把 HTTP 元数据塞进公开 JSON extra 字段。

**验证**：真实客户端完整调用链、并发集成测试、不同版本/重复 beta 的 identity 检查、客户端凭据隔离断言。

### 2. 分开识别 effort 与结构化输出

**已确认事实**：普通 Claude Code 推理请求就带 `output_config.effort`，并不一定带 `output_config.format`。原来只要出现 `output_config` 就判定用了 structured outputs。

**改动**：只将 `output_config.format` 视为结构化输出，单独识别 `output_config.effort` 为 reasoning effort。

**验证**：不支持 structured outputs、支持 reasoning effort 的 capability fixture 接受仅有 effort 的真实请求形状；加入 format 后仍拒绝。未扩展模型原本声明的功能。

### 3. 采用客户端的会话身份

**已确认事实**：样本中 `metadata.user_id` 是 JSON 字符串，含 `device_id`、`account_uuid` 和 `session_id`。原来的 ChatGPT 路径只读取独立 metadata/extra 中的会话字段，随后根据 system 和首个用户消息合成 ID。

**改动**：ChatGPT 在显式 conversation/thread/session identity 均缺失时，读取 JSON 字符串中的非空 `session_id`，供现有 continuation、context usage 和 prompt cache 作用域使用。仅有用户/设备信息、损坏 JSON 或无效 session_id 时沿用合成路径。没有将设备 ID 当成会话 ID。

**推断与验证边界**：首条消息或 system 在压缩后改变可能导致旧合成 ID 改变。回归测试验证同一个真实 session_id 在不同首条消息下保持身份，不同 session_id 分离，以及显式 identity 优先；本轮没有跑真实模型的自动压缩或测量缓存收益。

### 4. 保留 thinking 显示选项

**已确认事实**：直连样本包含 `thinking.display = updates`，修复前经 aisub2api 后该字段消失。

**改动**：在 `ThinkingConfig` 中保留可选的 `display`；未提供时不新增该字段。转换到其他协议时仍由对应 provider 处理自身的 reasoning 语义。

**验证**：请求序列化 round trip、Anthropic 集成请求体断言、修复后真实客户端调用链的上游字段对比。

## 验证记录

- `cargo test --locked -p aisub2api-core -p aisub2api-providers -p aisub2api-server -- --quiet`：550 项通过（core 12、providers 404、server unit 114、server integration 20）。
- `cargo clippy --locked --workspace --all-targets -- -D warnings`：通过。
- `cargo fmt --all --check`、`git diff --check`：通过。
- `cargo build --locked -p aisub2api-cli`：通过。
- 最终构建运行五组真实客户端经代理实验，全部通过；额外断言客户端完成标记、`thinking.display`、协议头、上游认证替换，以及工具结果回传和 context management 降级。
- 代码改动已人工检查相关调用方、provider 默认委派、去重 identity 和序列化边界。其他测试文件中的 ThinkingConfig 默认值补齐仅用于适配新增可选字段。

## 未覆盖项与重新评估条件

- **REA 静态调用链**：模块表已能解析，下一步可按当前二进制摘要定向读取所需模块 contents，并为共享 bytecode 保留引用，避免重复输出。可读业务逻辑的覆盖率和字节码反编译仍待验证；不以字符串命中推断分支行为。
- **MCP tool search**：当前场景只用内置 Read，未配置 MCP；待准备受控 MCP fixture 后验证 `defer_loading`、`tool_reference` 和拒绝降级，不根据旧笔记默认宣称支持。
- **真实长会话与自动压缩**：待有明确的真实 provider 验证范围与 token 预算后测量；本轮只验证身份约束和客户端 context management 拒绝恢复。
- **精确 token counting**：当前没有 `/v1/messages/count_tokens`。只有能使用与上游匹配的计数能力时才增加端点，不用字符估算冒充精确计数。
- **模型服务端功能**：本地 fixture 不证明真实 Anthropic beta、ChatGPT continuation、thinking 签名校验或账户权限可用；需要按具体上游再验证。

当前接口依据还包括 Anthropic 的 [gateway compatibility guide](https://code.claude.com/docs/en/llm-gateway-protocol)。例如，当前客户端可通过 `CLAUDE_CODE_ENABLE_FINE_GRAINED_TOOL_STREAMING=1` 在自定义地址下发送 `eager_input_streaming`；已在 Read 场景观测到，与 5 月旧调研中的限制不同。

## 本地复核产物

临时目录 `/tmp/aisub2api-rea-20261105` 中保留：

- `toolchain.json`：固定版本与摘要。
- `claude-artifact.json`：REA 原生提取失败结果。
- `bun-graph-diagnostics.json`：模块表、重叠区间覆盖量和原始拒绝位置的只读诊断。
- `claude-code-local.har`、`claude-capture-evidence.json`：协议样本与 REA Evidence。
- `captures/`、`captures-proxy/`：五组直连/经代理请求、模拟响应、客户端输出和摘要。
- `probe_claude.py`：本次隔离实验脚本；可用 `--proxy-binary <本次构建的 aisub2api>` 复核经过代理的调用链。

临时产物未加入版本控制。仓库中的 `tests/fixtures/claude_code/messages_request.json` 是根据真实样本裁剪、替换提示/工具描述/身份后的协议 fixture，不是原始流量。未 push 或部署。
