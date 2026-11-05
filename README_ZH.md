# aisub2api

**将 ChatGPT/Codex、GitHub Copilot 等 AI 订阅转换为兼容 API。**

> Turn the AI subscriptions you already pay for into local Anthropic and OpenAI compatible APIs.

[![CI](https://github.com/MorseWayne/aisub2api/actions/workflows/ci.yml/badge.svg)](https://github.com/MorseWayne/aisub2api/actions/workflows/ci.yml)
[![Release](https://github.com/MorseWayne/aisub2api/actions/workflows/release.yml/badge.svg)](https://github.com/MorseWayne/aisub2api/actions/workflows/release.yml)
[![GitHub Release](https://img.shields.io/github/v/release/MorseWayne/aisub2api?display_name=tag)](https://github.com/MorseWayne/aisub2api/releases/latest)
[![Rust](https://img.shields.io/badge/built_with-Rust-dca282.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](#许可证)

[English](README.md) · [立即安装](#安装) · [快速开始](#快速开始) · [查看 Provider](#支持的-provider)

`aisub2api` 是本地运行的 AI 订阅转 API 工具。它通过 OAuth 接入 ChatGPT/Codex 和 GitHub Copilot 账号，并暴露 Anthropic Messages、OpenAI Chat Completions 和流式 Responses API；API key Provider 与多模型路由作为补充能力保留。项目以单个 Rust 原生二进制运行，零运行时依赖。

## 为什么选择 aisub2api

- **订阅直接转 API**：通过 OAuth 使用已有 ChatGPT/Codex 和 GitHub Copilot 订阅，无需为同一能力额外维护 API key。
- **Agent 协议适配**：理解 Anthropic Messages、OpenAI Chat Completions/Responses、tool call、thinking、流式事件和 Codex 请求语义，不是简单 HTTP 转发。
- **多 Provider 路由**：同一入口也可接入 OpenAI、Anthropic、OpenRouter、Google 和私有兼容服务。
- **Claude Code 上下文增强**：为符合条件的 ChatGPT 模型提供虚拟 1M 投影、GPT-5.6 默认 872K 上游窗口、本地上下文预检和自动压缩信号。
- **按模型能力配置推理**：模型别名、推理强度和能力约束由真实模型元数据驱动，避免发送不支持的参数。
- **可观察、可运营**：内置 TUI Dashboard、SQLite 历史指标、限流、并发控制、热重载、守护进程和请求去重。
- **单文件部署**：原生 Rust 二进制，零运行时依赖，适合个人工作站和团队网关。

## 安装

Linux / macOS:

```bash
curl -fsSL https://github.com/MorseWayne/aisub2api/releases/latest/download/install.sh | bash
```

Windows PowerShell:

```powershell
irm https://github.com/MorseWayne/aisub2api/releases/latest/download/install.ps1 | iex
```

或从 [GitHub Releases](https://github.com/MorseWayne/aisub2api/releases) 下载预编译二进制。Linux x86_64 安装脚本默认下载静态链接的 `x86_64-unknown-linux-musl` 版本；`x86_64-unknown-linux-gnu` 版本仍可在 Releases 中手动下载。

### 从旧名称升级

安装脚本会在替换二进制后立即迁移旧状态，并移除安装目录中的旧 `claude-proxy` 可执行文件。如果新目录尚不存在，旧名称下的配置、OAuth token、metrics、日志和模型能力缓存会原子迁移到 `aisub2api`；如果新旧目录同时存在，`aisub2api` 目录保持权威，旧目录会原子归档为 `claude-proxy.legacy-backup*`，既不影响启动也保留人工恢复的可能。未通过安装脚本部署时，同一迁移会在首次运行新版程序时执行。升级前应先停止旧守护进程。

## 快速开始

```bash
# 添加 OpenAI provider（交互式输入 API key，并自动拉取模型列表）
aisub2api provider add openai

# 添加 GitHub Copilot provider（自动引导 OAuth 认证）
aisub2api provider add copilot

# 添加 ChatGPT provider（使用 ChatGPT Pro/Plus 账号 OAuth 认证）
aisub2api provider add chatgpt

# 启动代理服务
aisub2api server start

# 将 Claude Code 指向本地代理
export ANTHROPIC_BASE_URL=http://127.0.0.1:8082
export ANTHROPIC_API_KEY=freecc
```

之后，Claude Code 发出的 Anthropic Messages API 请求会进入 `aisub2api`，再按配置路由到对应上游模型。

## 界面预览

在一个 TUI 中管理 Provider、模型路由、OAuth 登录和运行指标：

![ChatGPT 订阅接入](images/tui-chatgpt-add-provider.svg)

<details>
<summary>查看更多 CLI、TUI 与 Claude Code 集成截图</summary>

### 快速接入 Claude Code

![快速接入流程](images/quickstart-flow.svg)

### CLI 总览

![CLI 总览](images/cli-overview.svg)

### 模型路由配置

![模型路由配置](images/tui-chatgpt-model-aliases.svg)

### 配置与服务控制

![配置与服务控制](images/config-server-commands.svg)

### ChatGPT OAuth

![ChatGPT OAuth](images/tui-chatgpt-oauth.svg)

### Metrics API

![Metrics API](images/metrics-api.svg)

</details>

## 支持的 Provider

| Provider | 认证方式 | 默认/典型端点 | 适用场景 |
|----------|----------|---------------|----------|
| OpenAI | API key | `https://api.openai.com/v1` | 使用 GPT 系列模型或 OpenAI 兼容模型 |
| Anthropic | API key | `https://api.anthropic.com` | 直接代理官方 Claude API |
| GitHub Copilot | OAuth | `https://api.githubcopilot.com` | 使用 Copilot 账号作为 Claude Code 上游 |
| ChatGPT | OAuth | `https://chatgpt.com/backend-api/codex` | 使用 ChatGPT Pro/Plus 账号能力 |
| OpenRouter | API key | `https://openrouter.ai/api/v1` | 通过统一市场访问多家模型 |
| Google | API key | `https://generativelanguage.googleapis.com/v1beta` | 接入 Gemini / Google 兼容接口 |
| Custom OpenAI | API key | 自定义 | 接入 one-api、new-api、LiteLLM、LocalAI、vLLM 等 OpenAI 兼容网关 |
| Custom Anthropic | API key | 自定义 | 接入 Anthropic 兼容私有网关 |

## 工作方式

```text
Claude Code / SDK / Client
        │
        │ Anthropic Messages API
        ▼
aisub2api /v1/messages
        │
        ├─ 认证：x-api-key 或 Authorization: Bearer
        ├─ 限流：按 API key 的 token bucket
        ├─ 并发：全局并发 + 单 Provider 并发
        ├─ 路由：model alias 或 provider_id/upstream_model
        ├─ 去重：相同并发请求共享同一个上游流
        ├─ 转换：OpenAI/兼容流转换为 Anthropic SSE
        └─ 指标：实时内存统计 + SQLite 历史持久化
        │
        ▼
OpenAI / Anthropic / Copilot / ChatGPT / OpenRouter / Google / Custom
```

### 模型路由

`[model].default` 使用 `provider_id/upstream_model` 格式：

```toml
[model]
default = "openai/gpt-4.1"
reasoning = "openai/o4-mini"
opus = "anthropic/claude-opus-4-20250514"
sonnet = "anthropic/claude-sonnet-4-20250514"
haiku = "anthropic/claude-haiku-4-5-20251001"
```

当客户端请求 Claude 模型名时，`aisub2api` 会优先匹配别名；没有命中时，则使用默认 provider 并把模型名原样传给上游。

## CLI 命令

### Provider 管理

```bash
aisub2api provider list              # 列出已配置的 provider
aisub2api provider current           # 显示当前默认模型
aisub2api provider add [id]          # 添加 provider（省略 ID 则交互式输入）
aisub2api provider edit <id>         # 编辑 provider 配置
aisub2api provider delete <id>       # 删除 provider
aisub2api provider switch <id>       # 切换默认模型到指定 provider
aisub2api provider test <id>         # 测试 API key 是否可用
aisub2api provider speedtest <id>    # 测试 provider 延迟
aisub2api provider fetch-models <id> # 拉取并缓存模型列表
```

### 配置管理

```bash
aisub2api config show                # 查看配置（密钥脱敏）
aisub2api config edit                # 用 $EDITOR 打开配置文件
aisub2api config validate            # 校验配置文件
aisub2api config path                # 打印配置文件路径
aisub2api config export [path]       # 导出配置（省略路径则输出到 stdout）
aisub2api config import <path>       # 从文件导入配置
```

### 服务管理

```bash
aisub2api server start               # 前台启动代理服务
aisub2api server start --daemon      # 以守护进程启动（仅 Unix）
aisub2api server stop                # 停止守护进程（仅 Unix）
aisub2api server restart             # 通过 SIGUSR1 重载配置（仅 Unix）
aisub2api server status              # 查看守护进程运行状态
```

### 本地维护

```bash
aisub2api logs                       # 实时打印日志（默认跟随当前日志文件）
aisub2api logs --file /path/app.log  # 跟随指定日志文件
aisub2api clean --yes                # 清空本地日志和 metrics.db（服务运行时需先停止或加 --force）
```


## TUI 终端控制台

```bash
aisub2api tui
```

TUI 提供 8 个页面：Dashboard、Providers、Server、Limits、HTTP、Logging、Model、System。

适合用它完成这些操作：

- 查看当前请求总数、错误数、平均延迟和各模型 token 用量。
- 查看 SQLite 持久化后的历史累计数据，重启后仍可追踪成本趋势。
- 管理 Provider、默认模型、服务器监听地址、限流、HTTP 超时和日志开关。
- 在终端里完成配置编辑，避免手动修改 TOML 时漏字段。

### 用 TUI 完整配置 ChatGPT

下面以 ChatGPT provider 为例，从零完成 provider、OAuth、模型路由、服务端口和 Claude Code 环境同步配置。

#### 1. 启动 TUI 并进入 Providers

```bash
aisub2api tui
```

- 用 `↑↓` 选择左侧菜单的 **Providers**，按 `Enter` 进入内容区。
- 在 Providers 页按 `a` 添加 provider。
- 在弹出的 **Add Provider — Select Type** 中选择 `ChatGPT — ChatGPT OAuth`，按 `Enter`。

![TUI 添加 ChatGPT Provider](images/tui-chatgpt-add-provider.svg)

#### 2. 完成 ChatGPT OAuth 登录

选择 ChatGPT 后，TUI 会自动创建 `chatgpt` provider，并立即进入 OAuth 设备码流程：

- 弹窗中会显示验证地址和一次性 user code。
- 按 `u` 可复制验证地址，按 `c` 可复制 user code。
- 在浏览器中打开验证地址，登录 ChatGPT Pro/Plus 账号并确认授权。
- 授权完成后，TUI 会自动保存 OAuth token；后续也可以在 Providers 页选中 `chatgpt` 后按 `o` 重新认证。

![TUI ChatGPT OAuth 设备码](images/tui-chatgpt-oauth.svg)

ChatGPT 使用 OAuth，因此 Providers 详情里的 **API Key** 会显示为 `OAuth (auto)`，不需要手动填写。通常只需要确认：

| 字段 | 推荐值 | 说明 |
|------|--------|------|
| `API Key` | `OAuth (auto)` | ChatGPT provider 自动维护 token |
| `Base URL` | `https://chatgpt.com/backend-api/codex` | 默认值即可 |
| `Proxy` | 留空或代理地址 | 需要 HTTP/SOCKS 代理时填写 |
| `Claude Context` | `AUTO` | 上游真实窗口大于 200k 时，让 Claude Code 使用虚拟 1M 窗口 |

如果需要修改 `Base URL` 或 `Proxy`：在 Providers 详情页按 `→` / `Enter` 进入详情，移动到字段后按 `e` 编辑，确认后按 `Ctrl+S` 保存。

使用 Pi 0.76.0+ 作为客户端时，自动化脚本可以用 `pi --session-id <id>` 固定项目内会话。aisub2api 会把安全的客户端 session header 转成 ChatGPT prompt-cache / continuation key，稳定 session id 有助于提高 ChatGPT/Codex 的缓存亲和性。

高级场景可以在 TOML 里覆盖 ChatGPT Codex 请求身份头；默认值通常不需要修改，默认会使用 Codex 风格的 `originator` 和 `User-Agent`。`User-Agent` 会优先使用本地 `codex --version` 探测到的版本；探测失败时使用内置默认值：

```toml
[providers.chatgpt.chatgpt]
# originator = "codex_cli_rs"
# user_agent = "codex_cli_rs/1.0.0 (aisub2api)"
# transport = "auto"       # "auto"、"websocket" 或 "sse"
# responses_lite = "auto"  # "auto"、"on" 或 "off"
# claude_code_context = "auto" # "standard" 可关闭虚拟 1M 投影

# 可选：在线 /models 目录不可用或需要本地强制覆盖时使用。
# GPT-5.6 默认采用 Codex 目录声明的 872K 支持上限；显式覆盖优先。
[providers.chatgpt.chatgpt.model_capabilities."gpt-5.6-sol"]
context_window = 872000
image_input = true
reasoning_effort_levels = ["low", "medium", "high", "xhigh", "max", "ultra"]
responses_lite = true
```

ChatGPT provider 会优先通过 OAuth 从 Codex `/models` 获取在线模型目录，并把最近一次成功获取的上下文能力写入 XDG cache 下的 `aisub2api/chatgpt/model-capabilities.json`。能力来源优先级为：显式 `model_capabilities` 覆盖 > 同 provider / Base URL / 身份摘要 / 模型的远端缓存 > 内置能力表。身份摘要包含可用的账号、用户、邮箱、套餐及额外路由头信息；已知身份的普通 token 轮换可以继续复用，身份改变或 provider 被替换后，旧刷新结果会被拒绝。持久化缓存已升级到 v2，旧条目会重新获取，token 存储格式不变。对 GPT-5.6，在线目录同时返回 `context_window` 与 `max_context_window` 时，默认采用后者的 872K 支持上限；其他模型仍优先使用 `context_window`。缓存不保存原始账号或 token，并采用临时文件加原子重命名写入。

当前内置 ChatGPT 回退能力如下：

| 模型 | 用途 | Context | 推理强度 | Responses Lite |
|------|------|---------|----------|----------------|
| `gpt-5.6-sol` | 默认 / Reasoning / Opus | 872k | `low`…`ultra` | 是 |
| `gpt-5.6-terra` | Sonnet | 872k | `low`…`ultra` | 是 |
| `gpt-5.6-luna` | Haiku | 872k | `low`…`max` | 是 |
| `gpt-6-astra` | 可选高级模型 | 272k | `low`…`ultra` | 是 |
| `gpt-6-sol` | 可选通用模型 | 272k | `low`…`ultra` | 是 |
| `gpt-6-luna` | 可选轻量模型 | 272k | `low`…`max` | 是 |
| `gpt-6.1-sol` | 可选通用模型 | 272k | `low`…`ultra` | 是 |
| `gpt-6.1-terra` | 可选通用模型 | 500k | `low`…`ultra` | 是 |

Astra、GPT-6 Sol/Luna 和 GPT-6.1 Sol 的 ChatGPT 回退使用 Codex 目录中的 272K active context；GPT-6.1 Terra 使用 500K active context，均不自动启用可选的 872K 扩展窗口。`openai/gpt-6-astra` 则声明公开 API 的 1,050,000 context、128,000 output 和 `low` 到 `max` 推理能力，Messages 请求也使用 Responses；两种提供商的限制分别处理。

升级后，ChatGPT Messages SSE 的输出前 overload 恢复、Anthropic 错误类型和 request ID 保留自动生效，无需迁移 token 或手工添加 Claude Code 环境变量。恢复窗口最多 5 秒，累计达到 64 个事件或 256 KiB 时交付；HTTP 和 SSE 共用重试次数，文本、thinking、工具调用或未知事件交付后不再透明重放。原生 Responses 和 WebSocket 保留现有恢复行为。

若要在 Claude Code 中启用并行工具调用，需要对支持的模型显式选择 Full。全局保持 `auto`，再按模型设置；Luna 保留 Lite：

```toml
[providers.chatgpt.chatgpt]
responses_lite = "auto"

[providers.chatgpt.chatgpt.model_capabilities."gpt-5.6-sol"]
responses_lite = false

[providers.chatgpt.chatgpt.model_capabilities."gpt-6.1-sol"]
responses_lite = false

[providers.chatgpt.chatgpt.model_capabilities."gpt-6-astra"]
responses_lite = false

[providers.chatgpt.chatgpt.model_capabilities."gpt-6-luna"]
responses_lite = true
```

合并到已有同名 table，保留其他 capability，不要重复追加 table。全局 `on/off` 会覆盖逐模型选择；同 provider 的原生 Responses 也会使用这份策略。模型访问权限和 Full 可用性仍由上游账号决定。[多模型验证及边界](docs/reviews/2026-11-06-codex-claude-code-parallel-recovery.md)包含真实 CC、MCP、缓存和 WS continuation 的实测范围。

所有新建 ChatGPT 映射默认使用 `high`。TUI 的 Reasoning picker 对所有 Provider 都按当前模型声明的 `reasoning_effort_levels` 过滤，并在打开时按需拉取、缓存模型能力；能力明确为空时只显示 `unset`，拿不到能力或使用目录外的自定义模型时才显示完整兼容列表。以 GPT-5.6 为例，Sol/Terra 显示 `low` 到 `ultra`，Luna 只显示 `low` 到 `max`，不会再给 Luna 显示 `default`、`none` 或 `minimal`。`unset` 用于清除代理侧强制覆盖。`ultra` 遵循 Codex 语义：上游请求发送 `max`；支持 `ultra` 的 Sol/Terra/Astra 模型且请求包含委派工具时，同时启用主动多代理指令，否则仅按 `max` 发送并记录警告。Luna 不支持 `ultra`，会降为 `max`。

#### Claude Code 虚拟 1M 与真实上游窗口

Claude Code 2.1.206+ 支持通过模型名 `[1m]` 后缀启用 1M 上下文协议。`claude_code_context = "auto"` 是默认值：对真实上下文大于 200k 的 ChatGPT 模型，TUI 保存时只在同步到 Claude Code 的五个模型环境变量上追加 `[1m]`；代理内部的 `ModelInfo.context_window` 仍保持上游真实值，例如 GPT-5.6 默认为 872k。设置为 `standard` 或在 Providers 页把 **Claude Context** 切到 `OFF` 可关闭投影。

代理不删除历史消息，也不向上游额外发起压缩请求。虚拟 1M 请求会在本地按内容估算 token；同一账号、模型和稳定 session 的连续请求优先采用“最近一次上游 usage + 新增 delta”，否则使用完整粗估，图片和文档按固定 2000 token 计入而不是按 base64 长度。接近真实窗口时，代理在上游请求前返回 Claude Code 可识别的 HTTP 400 `invalid_request_error`：

```text
Prompt is too long: N tokens > M maximum safe input (model context window: W; local estimate based on SOURCE)
```

普通且有可压缩历史的请求、以及压缩后的续接请求，安全输入线为 `真实窗口 - 33k`；压缩摘要生成和没有可压缩历史的请求使用 `真实窗口 - 20k`。对 872k 模型分别是 839k 和 852k。Claude Code 收到提示后负责生成摘要并续接；若本地估算漏过，上游真正的 prompt-too-long 也会归一化成同类 HTTP 400，但 `input + max_tokens` 的输出预算错误保持独立恢复语义。TUI Model 页会显示虚拟窗口、真实窗口和两条阈值，Dashboard 会统计 1M 请求、本地拦截和上游漏拦截。

#### 3. 设置默认模型和 Claude 模型别名

进入左侧 **Model** 页面，为 Claude Code 常用模型名配置路由。字段格式都是：

```text
provider_id/model_name
```

ChatGPT provider 的 provider id 默认为 `chatgpt`。新建 provider 时会自动采用分层映射：

```toml
[model]
default = { name = "chatgpt/gpt-5.6-sol", reasoning_effort = "high" }
reasoning = { name = "chatgpt/gpt-5.6-sol", reasoning_effort = "high" }
opus = { name = "chatgpt/gpt-5.6-sol", reasoning_effort = "high" }
sonnet = { name = "chatgpt/gpt-5.6-terra", reasoning_effort = "high" }
haiku = { name = "chatgpt/gpt-5.6-luna", reasoning_effort = "high" }
```

在 TUI 中编辑方式：

- 在 **Model** 页选择 `Default`、`Reasoning`、`Opus Alias`、`Sonnet Alias` 或 `Haiku Alias`。
- 按 `e` / `Enter` 后，先选择 `chatgpt` provider。
- TUI 会拉取模型列表；选择目标模型后按 `Enter` 写入字段。
- 重复设置需要的 alias，最后按 `Ctrl+S` 保存。

![TUI 设置 ChatGPT 模型别名](images/tui-chatgpt-model-aliases.svg)

> 如果拉取模型列表失败，先回到 Providers 页选中 `chatgpt` 按 `t` 做连通性检查；如果 OAuth 过期，按 `o` 重新登录。

#### 4. 配置本地服务和 Claude Code 连接参数

进入 **Server** 页面确认本地代理监听参数：

| 字段 | 示例 | 说明 |
|------|------|------|
| `Host` | `127.0.0.1` | 只给本机 Claude Code 使用时推荐保留本地地址 |
| `Port` | `8082` | 本地代理端口 |
| `Auth Token` | `freecc` | Claude Code 连接本地代理时使用的 API key |
| `Admin Token` | 留空或自定义 | 管理接口 token；留空时复用 `Auth Token` |

按 `Ctrl+S` 保存后，TUI 会写入 `~/.config/aisub2api/config.toml`，并同步 Claude Code 配置里的环境变量：

```json
{
  "env": {
    "ANTHROPIC_BASE_URL": "http://127.0.0.1:8082",
    "ANTHROPIC_API_KEY": "freecc",
    "ANTHROPIC_MODEL": "chatgpt/gpt-5.6-sol[1m]",
    "ANTHROPIC_REASONING_MODEL": "chatgpt/gpt-5.6-sol[1m]",
    "ANTHROPIC_DEFAULT_OPUS_MODEL": "chatgpt/gpt-5.6-sol[1m]",
    "ANTHROPIC_DEFAULT_SONNET_MODEL": "chatgpt/gpt-5.6-terra[1m]",
    "ANTHROPIC_DEFAULT_HAIKU_MODEL": "chatgpt/gpt-5.6-luna[1m]",
    "CLAUDE_CODE_ATTRIBUTION_HEADER": "0",
    "CLAUDE_CODE_MAX_OUTPUT_TOKENS": "128000"
  }
}
```

#### 5. 启动服务并验证

退出 TUI 后启动代理：

```bash
aisub2api server start
```

再用 Claude Code 或任意 Anthropic Messages API 客户端访问本地代理。请求模型名可以直接使用：

- `chatgpt/gpt-5.6-sol` / `terra` / `luna`：精确路由到 ChatGPT provider 的对应能力层。
- `opus` / `sonnet` / `haiku` / `reasoning`：由上面的 alias 路由到 ChatGPT。

如果要确认配置是否生效：

```bash
aisub2api provider list
aisub2api provider current
aisub2api provider test chatgpt
aisub2api config show
```

常用按键速查：

| 按键 | 作用 |
|------|------|
| `↑↓` / `j k` | 移动选择 |
| `←→` / `h l` | 左侧菜单和内容区切换；Providers 页也用于列表/详情切换 |
| `a` | 添加 provider |
| `e` / `Enter` | 编辑当前字段或进入详情 |
| `t` | 测试当前 provider |
| `o` | OAuth provider 重新认证 |
| `Space` | Logging 页切换布尔开关 |
| `Ctrl+S` | 保存配置并同步 Claude Code 环境变量 |
| `Esc` / `q` | 返回或退出；有未保存改动时会提示保存 |

## 配置文件

路径：`~/.config/aisub2api/config.toml`

```toml
[providers.openai]
api_key = "sk-..."
base_url = "https://api.openai.com/v1"
proxy = ""                              # 可选，HTTP 代理地址
provider_type = "openai"                # 可选；省略时按 provider ID 推断

[providers.openrouter]
api_key = "sk-or-..."
base_url = "https://openrouter.ai/api/v1"
provider_type = "openrouter"

[providers.anthropic]
api_key = "sk-ant-..."
base_url = "https://api.anthropic.com"
provider_type = "anthropic"

# GitHub Copilot provider（OAuth 自动认证，无需 api_key）
[providers.copilot]
base_url = "https://api.githubcopilot.com"
provider_type = "copilot"

[providers.copilot.copilot]
oauth_app = "vscode"                    # OAuth 应用: "vscode" 或 "opencode"
small_model = "gpt-5-mini"              # warmup 降级模型
max_thinking_tokens = 16000              # 最大思考 token 数
enable_warmup = true                     # 启用 warmup 检测（无工具请求自动降级）
enable_tool_result_merge = true          # 启用 tool_result 合并
enable_compact_detection = true          # 启用 compact/auto-continue 检测
enable_agent_marking = true              # 启用子 agent 流量标记
enable_responses_api = true              # 对 Copilot 启用 Responses API 路径

# ChatGPT provider（OAuth 自动认证，无需 api_key）
[providers.chatgpt]
base_url = "https://chatgpt.com/backend-api/codex"
provider_type = "chatgpt"

[providers.chatgpt.chatgpt]
# originator = "codex_cli_rs"
# user_agent = "codex_cli_rs/1.0.0 (aisub2api)"

[model]
default = "openai/gpt-4.1"
reasoning = "openai/o4-mini"
opus = "anthropic/claude-opus-4-20250514"
sonnet = "anthropic/claude-sonnet-4-20250514"
haiku = "anthropic/claude-haiku-4-5-20251001"

[server]
host = "127.0.0.1"
port = 8082
auth_token = "freecc"                   # 客户端连接所需的 API key
sse_heartbeat_interval_seconds = 15     # 下游 SSE keepalive 心跳间隔
stream_idle_timeout_seconds = 120       # 上游流无事件超时
stream_overall_timeout_seconds = 600    # 单次流式请求总超时
tool_use_terminal_timeout_seconds = 30  # tool_use 开始后等待 message_stop 的超时

[admin]
auth_token = ""                         # 留空时使用 server.auth_token

[limits]
rate_limit = 240                        # 时间窗口内最大请求数
rate_window = 60                        # 时间窗口（秒）
max_concurrency = 5                     # 全局最大并发请求数
max_concurrency_queue = 32              # 全局并发等待队列长度
provider_max_concurrency = 4            # 单个上游 provider 最大并发请求数
provider_max_concurrency_queue = 16     # 单个 provider 并发等待队列长度
model_cache_ttl_seconds = 3600          # 上游模型列表缓存时长（秒）
max_non_stream_response_bytes = 33554432 # stream=false 保留的逻辑 ProviderEvent 上限（32 MiB）

[http]
read_timeout = 300                      # 上游读取超时（秒）
write_timeout = 60                      # 上游写入超时（秒）
connect_timeout = 60                    # 上游连接超时（秒）
extra_ca_certs = []                     # 额外 CA 证书路径，适合企业 TLS 代理
max_response_body_bytes = 16777216      # 上游成功非 SSE body 上限（16 MiB）
max_sse_frame_bytes = 1048576           # 上游单个未闭合 SSE frame 上限（1 MiB）

[log]
level = "info"
file = ""                               # 默认写入 config_dir/aisub2api.log
with_stdout = true
raw_api_payloads = false                # 调试时再开启，可能包含敏感信息
raw_sse_events = false
```

响应边界默认启用，单位均为字节。非流式响应超限时会在提交下游响应头前返回上游
`502` 错误；流式 SSE frame 超限时，由于 HTTP 流已开始，会通过对应协议的带内错误
终止。非流式预算统计 normalized/native 事件实际保留的逻辑载荷，不等同于 allocator
的精确堆内存用量。

## HTTP API

### 代理接口

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/health` | 健康检查 |
| `POST` | `/v1/messages` | Anthropic Messages API 代理 |
| `POST` | `/v1/chat/completions` | OpenAI Chat Completions 兼容代理 |
| `POST` | `/v1/responses` | Codex 原生流式 Responses 代理 |
| `GET` | `/v1/responses` | 返回 `426`，让 Codex 从 WebSocket 降级到 HTTP |
| `GET` | `/v1/models` | 获取可用模型列表 |

Chat Completions 为已配置的 Provider 保留流式和非流式兼容。Responses 接口的范围
更窄：必须使用 `stream: true`，并且只能路由到原生 OpenAI 或 ChatGPT Responses
Provider。请求项和 SSE 事件不会先转换成 Anthropic Messages，因此会保留 Responses
Lite、configuration update、带 namespace 的工具输出和 V2 compaction 项。

Responses 接口仍按无状态模式工作：`store: true`、`background: true`、
`previous_response_id` 和 `conversation` 会明确返回错误。暂不暴露下游 Responses
WebSocket；GET 探测会返回 `426 Upgrade Required`，让 Codex 立即改走 HTTP。

`GET /v1/models` 会在 `capabilities.responses` 中提供按 Provider 区分的 Responses
约束，包括必须使用的流式模式、无状态与存储行为、支持的输入形态、Structured
Outputs 能力和不支持的参数。每个模型还会返回 `provider` 和可直接路由的
`qualified_id`，调用方无需再从未限定的模型名猜测 Provider 能力。

对于 ChatGPT Provider，字符串 `input` 会自动归一化为输入项列表，Responses Lite
要求的 `reasoning.context=all_turns` 和 `parallel_tool_calls=false` 也会自动补齐。
Codex 后端不接受 `max_output_tokens`；Proxy 会在 `unsupported_parameters` 中声明并在
转发前移除。原生 OpenAI Provider 仍会原样收到 `max_output_tokens`。

服务档位只在请求或配置指定时发送；显式 `flex` 不会因为目录缺少该档位而被移除，目录中的默认档位提示也不会隐式启用 priority。Flex 容量不足在开流前返回 HTTP 503 和 `x-should-retry: false`。

Responses 错误会保留服务端重试建议和请求 ID。HTTP 等待建议超过 5 秒时，代理交还调用方安排重试，不提前重试。未知的不完整响应原因在 Messages/Chat 翻译路径报告为错误；原生 Responses 保留原始终止事件。长度截断和内容过滤保持已有结束原因映射，用量仍被记录。收到终止 SSE 事件后立即结束上游读取。

模型不存在和音频校验错误在 HTTP、SSE、WebSocket 中均按终止错误处理。策略限制路由容量不足仍可重试，下游返回 529 并保留上游 `Retry-After` 建议。

最后一个流式消费者断开后，Proxy 会立即停止上游读取；仍有其他共享消费者时继续传输。ChatGPT 客户端仅在已知 HTTPS/WSS ChatGPT 域名存储上游 `__oailb` 路由 Cookie，并在同一 provider 的 HTTP 与 WebSocket 之间共享，遵守域名、路径、安全传输和过期规则。WebSocket 连接及续接状态按凭证所有者和路由配置隔离。

使用 `transport = "auto"` 时，一次允许 fallback 的 WebSocket 失败会让该 provider 实例持续使用 HTTP/SSE，直到 provider 重载；不再维护定时恢复状态，也不再发送旧的 `generate=false` prewarm 请求。

### OpenAI 客户端接入

将代理的 `/v1` 地址设置为 OpenAI Base URL，并使用 `server.auth_token` 作为
API Key。模型名可以使用下面的 `provider_id/model_name` 完整形式，也可以使用
`[model]` 中配置的别名。

```bash
export OPENAI_BASE_URL=http://127.0.0.1:8082/v1
export OPENAI_API_KEY=your-server-auth-token
```

Chat Completions：

```bash
curl "$OPENAI_BASE_URL/chat/completions" \
  -H "Authorization: Bearer $OPENAI_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "openai/gpt-4.1",
    "messages": [{"role": "user", "content": "你好"}],
    "stream": false
  }'
```

Responses：

```bash
curl -N "$OPENAI_BASE_URL/responses" \
  -H "Authorization: Bearer $OPENAI_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "openai/gpt-4.1",
    "input": "你好",
    "stream": true
  }'
```

Python SDK：

```python
from openai import OpenAI

client = OpenAI(
    base_url="http://127.0.0.1:8082/v1",
    api_key="your-server-auth-token",
)

response = client.chat.completions.create(
    model="openai/gpt-4.1",
    messages=[{"role": "user", "content": "你好"}],
)
print(response.choices[0].message.content)
```

JavaScript SDK：

```javascript
import OpenAI from "openai";

const client = new OpenAI({
  baseURL: "http://127.0.0.1:8082/v1",
  apiKey: "your-server-auth-token",
});

const stream = await client.responses.create({
  model: "openai/gpt-4.1",
  input: "你好",
  stream: true,
});
for await (const event of stream) {
  console.log(event);
}
```

### 管理接口

管理接口需要 `Authorization: Bearer <admin_token>`。如果未设置 `admin.auth_token`，会使用 `server.auth_token` 作为 fallback。

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/admin/config` | 获取当前配置（密钥脱敏） |
| `PUT` | `/admin/config` | 更新配置（请求体：`{"config": "<toml>"}`） |
| `POST` | `/admin/restart` | 从磁盘重新加载配置 |
| `GET` | `/admin/metrics` | 获取请求指标（含全量历史数据） |

`GET /admin/metrics` 返回 JSON：

```json
{
  "requests_total": 42,
  "errors_total": 1,
  "avg_latency_ms": 320,
  "models": {
    "openai/gpt-4.1": {
      "requests": 30,
      "input_tokens": 15000,
      "output_tokens": 8000,
      "cache_creation_input_tokens": 0,
      "cache_read_input_tokens": 2000
    }
  },
  "diagnostics": {
    "errors": 1,
    "terminal_reasons": {
      "provider_error": 1
    },
    "error_kinds": {
      "rate_limited": 1
    }
  },
  "observability": {
    "summary": {
      "requests": 42,
      "errors": 1,
      "avg_total_latency_ms": 320,
      "avg_upstream_connect_ms": 80,
      "max_event_gap_ms": 120,
      "idle_gap_count": 0,
      "prompt_too_long_retries": 0
    }
  },
  "stored": {
    "requests_total": 1500,
    "errors_total": 12,
    "avg_latency_ms": 305,
    "models": {},
    "diagnostics": {}
  }
}
```

- 顶层字段是当前进程会话内的统计。
- `avg_latency_ms` 表示已完成请求的端到端耗时；流式请求会在 stream 结束后计入。
- `diagnostics` 按结束原因和错误类型聚合错误请求，方便定位 auth、rate limit、stream 等问题。
- `observability.summary` 汇总请求阶段耗时、上游连接耗时、最大事件间隔和 prompt-too-long retry 次数。
- `active_streams` 显示当前未结束的流式请求（仅 request/provider/model/耗时/最后事件类型，不包含 prompt 内容），用于诊断长会话卡住位置。
- `stored` 是 SQLite 持久化的历史累计，跨重启保留。
- TUI Dashboard 会合并实时数据和历史数据展示总计。

## 特性清单

| 能力 | 说明 |
|------|------|
| Anthropic Messages API 兼容 | 对 Claude Code/SDK 暴露熟悉的 `/v1/messages` 接口 |
| OpenAI 流式转换 | 将 OpenAI Chat Completion SSE 转为 Anthropic SSE |
| Anthropic 透传 | 支持官方 Anthropic API 和自定义 Anthropic 兼容端点 |
| Copilot OAuth | 自动完成 GitHub OAuth，模拟 VS Code/Copilot 请求头 |
| ChatGPT OAuth | 使用 ChatGPT 账号 device flow，token 保存在本机配置目录 |
| Provider 模型缓存 | 启动时预热模型列表，TUI/CLI 可复用缓存 |
| 重复请求去重 | 相同并发请求共享一次上游调用，减少浪费 |
| 限流与并发 | API key 级限流、全局并发、单 Provider 并发保护 |
| 热重载 | 文件监听和 SIGUSR1 均可触发配置 reload |
| 持久化指标 | 请求、错误、延迟、模型 token 用量写入 SQLite |
| TUI Dashboard | 终端内查看实时/历史统计和配置状态 |
| 企业网络支持 | Provider 代理和 `extra_ca_certs` 适配企业 TLS 拦截环境 |

## 与其他开源工具对比

| 工具 | 核心定位 | aisub2api 的差异 |
|------|----------|---------------------|
| LiteLLM Proxy | 通用 LLM 网关，覆盖大量模型 API | aisub2api 聚焦把 ChatGPT/Codex、Copilot 等订阅账号通过 OAuth 暴露为兼容 API，并保留 agent 协议语义 |
| OpenRouter | 托管式多模型路由平台 | aisub2api 直接复用本地订阅账号，也可把 OpenRouter 作为补充上游 |
| LocalAI / vLLM | 本地模型推理服务 | aisub2api 不负责推理，而是统一转发到云端、账号型或私有兼容上游 |
| one-api / new-api | 多渠道 API 分发与管理面板 | aisub2api 更适合开发者本机的订阅转 API 场景，内置 ChatGPT/Copilot OAuth 与 agent 协议适配 |
| 简单反向代理 | HTTP 转发 | aisub2api 会理解 Anthropic/OpenAI 协议、模型路由、流式事件、token 指标和 Provider 生命周期 |

如果你已经有 LiteLLM、one-api、new-api 或自建 OpenAI 兼容网关，可以继续把它们作为 API-key 上游；`aisub2api` 的核心价值仍是把订阅账号转换成 Claude Code、Codex 等客户端可使用的兼容 API。

## 典型使用场景

- **订阅账号转 API**：通过 OAuth 把 Copilot / ChatGPT 账号能力暴露为 Anthropic 或 OpenAI 兼容接口。
- **Claude Code 多模型切换**：在一个终端会话里用不同 alias 切换 GPT、Claude、Gemini 或 OpenRouter 模型。
- **团队内统一出口**：给团队提供一个 Anthropic 兼容入口，同时在本地记录用量和延迟。
- **企业网络环境**：通过代理和额外 CA 证书访问上游模型服务。
- **调试模型成本**：用 TUI Dashboard 观察模型维度 token 用量和错误率。

## 从源码构建

一条命令完成 Release 编译并安装到本地：

```bash
cargo install-local
```

Cargo 会将 `aisub2api` 安装到 `$CARGO_HOME/bin`（通常是 `~/.cargo/bin`）。
如需安装到其他根目录，例如 `~/.local/bin`，可以运行：

```bash
CARGO_INSTALL_ROOT="$HOME/.local" cargo install-local
```

该快捷命令等价于
`cargo install --path crates/aisub2api-cli --locked --force`；Cargo 的 install 命令默认使用 Release profile 编译，提交到仓库的 lockfile 用于保证本地安装结果可复现。

如果只需编译而不安装：

```bash
cargo build --release
# 二进制文件位于 target/release/aisub2api
```

构建 Linux musl 静态链接版本：

```bash
rustup target add x86_64-unknown-linux-musl
cargo build --release --target x86_64-unknown-linux-musl
# 二进制文件位于 target/x86_64-unknown-linux-musl/release/aisub2api
```

常用开发命令：

```bash
cargo test
cargo clippy -- -D warnings
cargo fmt --check
```

## 许可证

MIT
