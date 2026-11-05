# Claude Code + 第三方 provider 使用指南

本指南基于 Claude Code 2.1.318 的实际请求与隔离实验。Claude Code 的本地工具、代理公开的协议、上游模型能力是不同层；成功返回文本不代表三层全部等价。[调研与验证范围](../reviews/2026-11-05-claude-code-third-party-experience.md)记录了版本、来源及实测结果。

## 只选择 provider 和主模型即可

在 TUI 或 CLI 中完成 provider 和主模型配置后，aisub2api 自动生成 Claude Code 的兼容配置：

| 自动处理 | 默认行为 |
| --- | --- |
| 主模型及角色 | 第三方 provider 的空白 Haiku/Sonnet/Opus 角色继承主模型 |
| 本地任务工具 | 默认恢复 TaskCreate/TaskGet/TaskList/TaskUpdate 的可用性 |
| 工具搜索 | 新配置使用 auto，由 CC 按工具定义大小决定是否延迟加载 |
| 输出预算 | 补充缺省值，保留已有用户上限 |
| 上下文 | 继续按已知 ChatGPT 模型能力自动选择现有投影策略，不给不同模型强塞同一个全局窗口 |
| 配置同步 | CLI 导入/切换 provider、TUI 保存及启动服务时自动同步 CC 设置 |

不需要先手写下面的环境变量。已有的显式工具开关、输出预算和自定义 picker 会保留；CLI 的 provider 配置尚未完成时跳过同步，CC JSON 语法或结构损坏时也不覆盖原文件。已经运行的 CC 对部分启动设置仍需重新打开进程，代理无法替客户端重新读取这些值。

## 只有需要角色分工时才单独选模型

`ANTHROPIC_MODEL` 只设置主模型。CC 还会解析 `haiku`、`sonnet`、`opus` 等角色；背景请求也可能使用 Haiku。只设置主模型可能让第三方上游收到并不提供的原生 Claude ID。模型角色及认证路径之间的关系见[官方 gateway guide](https://code.claude.com/docs/en/llm-gateway-protocol)。

在 aisub2api 的配置或 TUI 中选择实际可用的模型，例如：

```toml
[model]
default = "chatgpt/gpt-6-astra"
haiku = "chatgpt/gpt-6-luna"
sonnet = "chatgpt/gpt-5.6-sol"
opus = "chatgpt/gpt-6-astra"
```

示例模型需替换为你的 provider/账号真正提供的模型。同步 Claude 配置时，显式角色配置优先；主 provider 不是 `Anthropic` 时，未配置或空白的 Haiku/Sonnet/Opus 角色继承主模型。直接配置的 `Anthropic` provider 保留其原生角色默认值。

继承只保证请求指向配置的模型，不会把它变成不同的能力或质量档位。需要低延迟的背景模型或不同的推理模型时，应显式配置对应角色；让背景请求继承昂贵主模型可能增加耗时或成本。

CLI 切换 provider 或导入配置、启动服务、用 TUI 保存模型设置都会同步 Claude 配置。修改模型配置后，新开 CC 进程核对实际模型；已经运行的 CC 不一定重新读取全部启动配置。

## 可选：覆盖自动设置

正常使用无需修改 CC 设置。如果你有特定的输出预算或工具策略，可以合并到 `~/.claude/settings.json` 的 `env`，保留原有字段：

```json
{
  "env": {
    "CLAUDE_CODE_MAX_OUTPUT_TOKENS": "32768",
    "ENABLE_TOOL_SEARCH": "auto"
  }
}
```

aisub2api 仅在没有输出上限设置时补充默认值；保存配置或启动服务不再覆盖你的已有上限。上限是单次生成预算，不是总 token 消耗，也不强制模型每次生成这么多；较低上限可能截断大型工具参数。effort 是独立设置，降低上限不等于降低 effort。

工具搜索可按实际问题选择：

| 设置 | 使用方式与代价 |
| --- | --- |
| `true` | 延迟加载工具，适合较大的 MCP 工具集；需要模型通过 ToolSearch 发现工具 |
| `auto` / `auto:5` | 依据工具定义大小决定；阈值受客户端认为的上下文窗口影响 |
| `false` | 直接加载工具，可用于排查发现失败；大量 MCP schema 会占用更多输入上下文 |

2.1.318 Linux 的受控主线程实验中三种模式都能调用 Read。不要仅因初始请求中没有某个 schema 就判断功能丢失，先检查搜索后的请求。遇到特定版本的工具发现故障时，可在新进程中试 `false`；[社区 #104309](https://github.com/anthropics/claude-code/issues/104309) 报告的主要环境为 macOS 2.1.306–2.1.310，不应直接当作本机版本的已知故障。aisub2api 对新配置补充 auto，并保留你已有的 `true/auto/false` 选择。

### 需要 TaskCreate / TaskUpdate 时

aisub2api 已自动补充以下设置，无需手动开启：

```json
{"env":{"CLAUDE_CODE_ENABLE_TODO_TOOLS":"1"}}
```

在本轮实验中，CC 自身对陌生模型默认不提供 TaskCreate；自动生成配置后，真实客户端直接完成 TaskCreate 和 Read。保留工具搜索 true 时也能先发现 TaskCreate 再创建任务。你已有的禁用值会保留；已有 `CLAUDE_CODE_ENABLE_TASKS=0` 时也保留旧 TodoWrite 选择。`ENABLE_TASKS` 只选择工具类型，不能替代恢复可用性的 `ENABLE_TODO_TOOLS`。参见[官方环境变量说明](https://code.claude.com/docs/en/env-vars)。这不改变执行权限，也不会自动启动子 agent。

### 需要细粒度工具参数流式时

OpenAI/ChatGPT 转换链路会按上游流式事件输出工具参数增量，不需要为这条链路额外开启 Anthropic 专用字段。原生 Messages 链路可以对已确认支持的上游测试 `CLAUDE_CODE_ENABLE_FINE_GRAINED_TOOL_STREAMING=1`，2.1.318 会在自定义地址下发送 `eager_input_streaming`；这个请求提示不是 OpenAI 上游的同名能力。aisub2api 保留用户开关，不对未知原生上游强加字段，也不保证打开开关就减少生成延迟。

## 自动上下文策略与高级覆盖

日常使用无需填写窗口变量。已知 ChatGPT 模型继续采用 aisub2api 原有的自动投影和真实上游窗口检查；未知容量保持保守行为。以下矩阵用于解释高级覆盖，不能据此把声明值当作上游实际容量。

以下是本地模拟请求中 CC 报告的窗口，不是上游可用容量的测量：

| 模型 ID 与设置 | CC 报告的窗口 |
| --- | --- |
| 陌生 ID，无额外配置 | 200,000 |
| 同一陌生 ID，`CLAUDE_CODE_MAX_CONTEXT_TOKENS=500000` | 500,000 |
| 陌生 ID 带 `[1m]`，同时声明 500000 | 1,000,000 |
| 上一项再设置 `CLAUDE_CODE_DISABLE_1M_CONTEXT=1` | 500,000 |
| 已识别的 Claude Sonnet 4.6 ID，只声明 500000 | 200,000 |

当前陌生 ID 可以独立声明窗口；不必照搬旧版建议，通过 `DISABLE_COMPACT=1` 换取覆盖。`[1m]`、已识别的 Claude ID 和裸陌生 ID 的规则不同；声明窗口不会扩大真实模型容量。官方说明见[模型窗口配置](https://code.claude.com/docs/en/model-config)。

`CLAUDE_CODE_MAX_CONTEXT_TOKENS` 是客户端全局值，不是按 provider/model 分别设置。多角色模型窗口不同，不能直接把主模型的大窗口套给所有模型。aisub2api 的 ChatGPT 虚拟 1M 投影是另一项策略，由代理按真实上游窗口处理；不要把它与上面的原生窗口覆盖混为一谈。本指南不自动改写窗口或关闭压缩。

## 模型列表与服务端功能

`/v1/models` 能列出模型，不代表 CC 会把所有模型显示在 `/model`。当前发现功能默认关闭，启用后还会过滤不含 `claude`/`anthropic` 的 ID。GPT 模型可通过完整 ID、角色映射、`ANTHROPIC_CUSTOM_MODEL_OPTION` 或用户自己的 `modelPicker` 选择；不要为了进入列表把 GPT 改名伪装成 Claude。aisub2api 同步时保留已有 `modelPicker`。[官方模型发现契约](https://code.claude.com/docs/en/llm-gateway-protocol#model-discovery)。

WebSearch 等工具的出现也不保证其服务端能力可用。可以选择确实支持相应工具的上游，或使用自己配置的搜索 MCP；后者是独立工具，不会自动等价于 Anthropic 服务端搜索。订阅云功能、Remote Control、云端任务和某些后台服务还依赖 Anthropic 登录与服务，API 转换无法补齐这些依赖。[官方功能矩阵](https://code.claude.com/docs/en/feature-availability)。

不要默认关闭全部 experimental betas：这会同时关闭部分有用能力，而且不保证移除 adaptive thinking 或 effort。仅在已确定某上游拒绝这些字段时选择兼容配置，优先保留原始错误让客户端按已支持的恢复路径降级。
