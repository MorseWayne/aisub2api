# Claude Code 工具搜索与错误恢复验证

日期：2026-11-05。基线：`2180a9d`。目标：定向读取 Claude Code 2.1.318 的可读模块，用受控 MCP 和模拟模型接口验证工具搜索、字段拒绝和代理错误恢复。

## 结论

- 定向读取 contents 已成功，得到 60 个候选 JS 模块，共 16,176,698 字节；未复制重复的 bytecode 区间，也未反编译字节码。
- 原生 Anthropic 路径能够保留 `defer_loading` 和 `tool_reference`，真实客户端完成了搜索、发现和 MCP 调用。
- 确认并修复一项错误契约缺口：上游 HTTP 422 原来变成下游 502，导致客户端无法按字段校验错误恢复，重复发送被拒绝的请求。
- 修复只保留 422 的状态与校验错误消息，使用已有错误 envelope；没有新增 capability、配置项、缓存或工具搜索实现。
- 本轮没有调用真实付费模型，也没有测量生产吞吐、模型质量或 token 成本。

## 静态证据与适用范围

继续使用首轮固定的二进制与 REA 源码：

| 对象 | 标识 |
| --- | --- |
| Claude Code | Linux 原生 `2.1.318` |
| 二进制 SHA-256 | `708c77b204db30086ff6f1ed34577bda03e1be74ced80557cbf7b54249ebf0bd` |
| REA commit | `71f680d7c1aa7a45ba0325fe87b104744b0c105f` |
| 运行时 | 隔离 Node `24.22.1` |

使用 REA 的 graph decoder 读取模块元数据，再按验证过的 contents 区间定向读取文件。原始模块名称、索引、byte range、内容摘要保存在临时 selection manifest 中；读取前重新核对二进制摘要。可读内容只写入临时目录，没有加入仓库。

以相关字段筛选出的 60 个模块不是完整应用。对其中八个小模块使用 `@babel/parser` 验证 AST，均能解析出函数和调用节点。之后通过 AST 定位工具搜索判断与错误恢复函数，再回到原始模块内容检查上下文。

关键位置：

- `chunk-5xgkngpb.js` 中 `kJr`，字符 offset `2452665..2454659`：判断工具搜索能否启用，包括模型、工具注册、会话已有状态和此前端点拒绝。
- 同模块的 `BTe`，起始 offset `1655321`：将 422 包装成 400 供部分恢复判断使用。
- 同模块请求构建逻辑中的 `nue`、`E0`：在已声明延迟工具时，检查 HTTP 400/422，触发去掉 deferred tools 的重试。相关逻辑还受其他能力恢复分支影响，不能仅根据这个局部条件预测全部请求次数。
- `chunk-tacvwrx2.js`：识别并收集 `tool_result.content` 中的 `tool_reference`。

这些位置依据的是固定版本的可读 JS 内容。混淆后的 symbol 与 offset 不作为跨版本契约，运行行为由下面的真实客户端实验验证。

REA 自身的 `analyze-javascript-application` 整组分析仍失败，错误为 `unreadable_output`，worker 在返回终结响应前关闭输出。该结果仅保留 partial observation，不当作完整调用图。因此本轮的 AST 定位使用独立 Babel parser，不宣称 REA 整包 JS 分析已修复。

## 受控 MCP 场景

本地 stdio MCP fixture 暴露 30 个虚构 lookup 工具，调用只返回固定文本。模拟模型按预定顺序请求 ToolSearch、调用发现的 MCP 工具、返回完成标记。

客户端使用隔离 `CLAUDE_CONFIG_DIR`、空 settings sources、显式 strict MCP 配置、`--restricted`、工具白名单及虚构 API key。仅连接回环接口，未使用已有登录凭据或生产服务。

ToolSearch 在本实验的 `--bare` 模式下没有进入工具列表，所以正式验证改用上述隔离 restricted 模式，并检查客户端实际发送的工具定义；最初未启用搜索的探测不计为工具搜索成功。

真实请求形状确认：

1. 初始请求只带 `DeferredToolPlaceholder` 和 ToolSearch，没有全部 30 个 MCP 工具 schema。
2. ToolSearch 的结果通过 `tool_result.content` 返回 `tool_reference`。
3. 后续请求带已发现 MCP 工具的 schema 及 `defer_loading = true`。
4. 关闭工具搜索后，请求中出现全部 30 个 MCP 工具，不再带 `defer_loading`，也不再请求对应工具搜索 beta。

因此，向其他协议转换时不能仅依据“存在 tools 数组”就假设完整工具定义已到达；本轮未更改 OpenAI/ChatGPT 的工具搜索支持声明或转换行为。

## 422 错误复现

上游模拟接口在请求仍带 deferred tool 时持续返回：

```json
{
  "type": "error",
  "error": {
    "type": "invalid_request_error",
    "message": "tools.0.defer_loading: Extra inputs are not permitted"
  }
}
```

只有客户端去掉延迟加载字段后，模拟接口才允许后续调用完成。回环 relay 记录客户端实际接收到的状态，区分上游错误和代理返回。

| 路径 | 客户端收到的状态 | 结果 |
| --- | --- | --- |
| 直连模拟接口 | `[422, 422, 422, 200, 200]` | 去掉延迟字段，完成 MCP 调用，退出 0 |
| `2180a9d` 构建的代理 | 七次 502 | 45 秒实验截止时仍未完成，脚本终止客户端并记录退出 124 |
| 修复后的代理 | `[422, 422, 422, 200, 200]` | 完成 MCP 调用，退出 0 |

三次 422 后才恢复是这个客户端和请求组合的观察结果，不是保证固定重试次数。不能将 45 秒实验截止描述为客户端永远不会恢复。

还验证了明确的 `capability_rejected: beta_header:advanced-tool-use-2025-11-20` 消息。该命名 token 在旧 502 envelope 下也能触发部分恢复，所以仅测试该消息会漏掉普通字段校验错误的状态码问题。

## 修复与错误契约

修复位置：

- `routes::upstream_error_to_response`：Anthropic Messages 下游保留 HTTP 422，将消息放入已有 `invalid_request_error` envelope。
- `downstream::provider_error_response`：Chat Completions 和 Responses 下游采用同样的 422 校验错误分类与消息。

状态来自 `ProviderError::UpstreamError.status`，错误文本继续从现有解析函数提取。保持现有认证、上游元数据及其他状态分类，不增加新的 error variant。变更发生在 HTTP 响应开始之前，不改变已开始的 SSE 内错误编码。

对调用方的变化是：上游 422 现在表现为输入校验失败，不再表现为 502 网关故障。客户端可调整能力字段后重发，HTTP SDK 也不会仅因这个状态将同一输入当作临时网关故障重试。没有声称每一种校验错误都可以自动恢复。

## 验证

- 修复前，新增 `upstream_validation_errors_remain_422_for_all_downstream_protocols` 测试失败：实际 502，期望 422。
- 修复后 `cargo test --locked -p aisub2api-server -- --quiet`：136 项通过（unit 115、integration 21）。
- 新集成测试覆盖真实 server/provider 边界，断言 422、`invalid_request_error` 和原始字段错误消息。
- 单元测试覆盖 Anthropic Messages、Chat Completions、Responses 三种下游 envelope。
- `cargo clippy --locked -p aisub2api-server --all-targets -- -D warnings`、`cargo build --locked -p aisub2api-cli`：通过。
- 四组真实客户端经代理实验全部通过：工具搜索成功、命名 beta 的 400 拒绝、命名 beta 的 422 拒绝、普通字段校验 422。
- 支持路径断言 `tool_reference` 保留；拒绝路径断言最终移除 deferred 字段及配套 beta、完整加载工具，且真实 MCP fixture 收到目标调用。

## 本地证据与剩余范围

临时目录仍为 `/tmp/aisub2api-rea-20261105`：

- `module-inventory.json`、`source-selection.json`、`source-ast-check.json`：元数据、选中模块与 AST 检查。
- `source-selected/`：定向内容；`ast-ENABLE_TOOL_SEARCH.json`、`ast-defer_loading.json`：定位结果。
- `claude-source-summary.json`：REA 整组 JS 分析失败及 partial observation。
- `mock_mcp.py`、`probe_tools.py`：本次受控 MCP 与协议实验脚本。
- `tool-search-field-direct.jsonl`、`tool-search-field-proxy-before.jsonl`、`field-422-before-client-responses.json`：修复前对照。
- `tool-search-proxy-after.jsonl`、`captures-proxy/*/client-responses.json`：修复后的客户端状态和请求记录。

可读模块和临时流量不进入仓库。后续仍需分别验证：OpenAI/ChatGPT 的工具搜索转换与显式降级、真实长会话压缩、流式断连恢复、模型端实际能力与成本。本轮没有新增这些能力，也没有修改模型支持矩阵。

外部契约参考：Anthropic [gateway compatibility guide](https://code.claude.com/docs/en/llm-gateway-protocol) 的工具搜索恢复与错误转发说明。行为结论以固定版本实验为准。
