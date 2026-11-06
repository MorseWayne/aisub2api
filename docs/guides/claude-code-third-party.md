# Claude Code + 第三方 provider 使用指南

本指南基于 Claude Code 2.1.318 的实际请求与隔离实验。Claude Code 的本地工具、代理公开的协议、上游模型能力是不同层；成功返回文本不代表三层全部等价。[调研与验证范围](../reviews/2026-11-05-claude-code-third-party-experience.md)记录了版本、来源及实测结果。

## 按功能异常定位配置

| 症状 | 对应配置或处理 | aisub2api 当前行为 |
| --- | --- | --- |
| 没有 TaskCreate / TaskUpdate | `env.CLAUDE_CODE_ENABLE_TODO_TOOLS=1` | 已自动补缺省值；保留显式禁用 |
| 主对话成功，切换角色或后台请求 model_not_found | 配置 Haiku / Sonnet / Opus 角色到实际可用 ID | 第三方未配置角色已自动继承主模型 |
| 工具发现失败或上游拒绝工具引用 | 排查 `ENABLE_TOOL_SEARCH`；必要时用 `false` 对照直接加载 | 默认 auto，保留已有选择 |
| WebFetch 报域名无法验证 | 检查 Anthropic 域名检查服务；必要时顶层设置 `skipWebFetchPreflight=true` | 不全局跳过域名检查，详见下文 |
| `/model` 没显示 GPT | 使用完整 ID、角色或自定义 picker；模型发现另有名称过滤 | 保留用户 picker；发现开关不能消除过滤 |
| 输出或上下文提前触顶 | 分别核对输出预算、真实模型窗口及代理投影 | 保留输出覆盖，按已知 ChatGPT 模型应用现有投影 |

[第三方差别待遇与功能开关核查](../reviews/2026-11-06-claude-code-chatgpt-restrictions.md)记录了每项证据及限制。功能异常优先对应具体配置，不需要把所有兼容开关一起打开。

### 先检查，再按需修复

```sh
aisub2api config validate
aisub2api config claude
```

`config validate` 校验代理配置，并只读检查 Claude 用户设置中的路由、凭据是否一致、任务工具、工具搜索、WebFetch 设置位置和已知无效开关。它显示实际检查的文件路径，不打印凭据或配置原值；项目设置、组织策略、shell / CLI 覆盖和运行中会话不在本次检查范围内。兼容性提醒不改变成功退出码，Claude JSON 损坏或 WebFetch 值类型错误会返回失败。

`config claude` 将已有代理配置重新同步到 Claude，补充缺省任务工具和搜索策略、更新模型及角色路由。已有的显式功能开关、picker、权限规则和 WebFetch 选择会保留。这个命令需要已完成的 provider / 主模型配置；它不会启动服务。

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

### WebFetch 报域名无法验证时

即使模型接到第三方接口，WebFetch 默认仍会先向 `api.anthropic.com` 检查目标域名。检查服务不可达或返回错误时，客户端会在请求目标网页前停止，常见错误为 `Unable to verify if domain ... is safe to fetch`。单独关闭 telemetry 或设置 `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1` 不会关闭该检查。

如果故障已定位到这个检查，使用显式修复入口：

```sh
aisub2api config claude --webfetch-preflight skip
```

恢复原有域名检查：

```sh
aisub2api config claude --webfetch-preflight check
```

带此选项的命令只修改 Claude 用户设置中的 WebFetch 域名检查，并清理误放在 env 中的同名项；不同步模型、认证或其他配置，也可在代理尚未配置时使用。后续常规配置同步保留这一选择。新开 Claude 会话核对效果。

对应 Claude settings 的顶层布尔设置如下；它与 `env` 同级：

```json
{
  "skipWebFetchPreflight": true
}
```

本轮模拟检查服务失败时，GPT 和原生 Claude 都抓取失败；设置该项后，GPT 路径恢复并且不再请求检查接口。它跳过 Anthropic 域名黑名单查询，抓取仍遵循 WebFetch 权限规则。此设置不提供 WebSearch，也不修复目标站点连接或模型摘要错误。aisub2api 不为所有第三方连接自动启用它。[官方说明](https://code.claude.com/docs/en/settings-reference#skipwebfetchpreflight)。

### capability 与 thinking 开关的适用范围

在本指南使用的普通 `ANTHROPIC_BASE_URL` 接入方式下，`ANTHROPIC_DEFAULT_*_MODEL_SUPPORTED_CAPABILITIES` 不参与能力覆盖。它们主要用于 Bedrock、Google Cloud、Foundry 等接入方式；给这里的 GPT ID 添加声明不会恢复功能，也不会按声明删掉其余能力。

2.1.318 的 `CLAUDE_CODE_DISABLE_ADAPTIVE_THINKING=1` 只对特定已知 Claude ID 生效，不能作为 GPT / 陌生模型的通用降级开关。本轮 GPT 仍发送 adaptive，Sonnet 4.6 对照则改成固定 thinking budget。`CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS=1` 会删除 context management 等字段，但仍可能保留 adaptive thinking、effort 和部分 beta header。上游拒绝字段时，应按具体协议与模型能力处理，不能仅凭开关名称认定已经降级。

关闭 telemetry、growthbook 或 nonessential traffic 还可能停止远端 feature flag 获取，让依赖它的部分功能不可用。这些设置不作为第三方接入的默认修复。[官方功能开关依赖](https://code.claude.com/docs/en/env-vars#features-that-need-feature-flag-fetching)。

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

## 升级 Claude 后运行本地兼容验收

仓库内脚本使用已安装的 Claude Code、新构建的 aisub2api、虚构凭据、临时配置以及本地 Messages / MCP / HTTPS fixture。需要 Python 3.10+、OpenSSL 和回环 socket 权限：

```sh
cargo build --locked -p aisub2api-cli
python3 -I scripts/claude-compat-smoke.py --aisub2api target/debug/aisub2api
```

可用 `--claude /path/to/claude` 指定客户端，`--output /tmp/claude-smoke.json` 保存摘要。摘要记录版本、两个可执行文件的 SHA-256 和逐项结果；失败返回非零。脚本有每次客户端调用的超时，清理临时目录、模拟服务和超时子进程，使用虚构凭据和独立配置，不安装客户端或复用现有登录凭据。

五组场景覆盖自动生成配置后的 TaskCreate → Read、ToolSearch → 实际 MCP 调用，以及 WebFetch 检查失败、显式跳过和恢复检查。WebFetch 失败对照必须观察到工具错误，不能只因最终会话成功就判定通过。HTTPS fixture 不转发外网；子进程使用单独的信任证书和代理配置，没有安装系统证书。它验证客户端配置和工具执行，不验证真实模型质量、账户权限或上游协议转换，也不代表操作系统级断网隔离。
