# Claude Code 第三方模型差别待遇与功能开关核查

日期：2026-11-06。对象：本机 Linux Claude Code 2.1.318。范围：`Claude Code CLI → aisub2api`，以及同一客户端发布产物中可定位的其他接入分支。

调研围绕两个目标：第一，检查第三方模型是否受到主动排除、额外功能限制或其他损害正常使用的区别对待；第二，确认哪些功能需要额外配置才能正常使用，哪些开关实际无效，以及 aisub2api 是否已经处理。名称是否被拒绝只是其中一个检查项。

## 结论

1. **存在明确识别其他厂商模型名称的代码。** 内置 gateway 的共享配置 schema 包含 `isDefinitelyNotAnthropicModel`、`isLikelyAnthropicModelId`、`checkModel`；其负向匹配包含 `gpt`、`openai`、`codex`，也包含 Gemini、DeepSeek、Qwen 等多种其他模型。不能概括为“完全没有针对 GPT 的检查”，也不能说它只针对 ChatGPT。
2. **本轮未发现这段检查限制普通 CLI 的显式模型请求。** 七种名称都完成了模拟模型驱动的 Read 调用。GPT、ChatGPT、OpenAI、Codex 与普通陌生 ID 的工具名称、thinking、effort、输出预算和 context management 完全一致。
3. **CLI 的模型发现确实过滤 GPT ID。** 这是一套独立的正向名称过滤：只保留含 `claude` 或 `anthropic` 的 ID。被过滤的 GPT ID 仍可通过显式 `--model` 使用。当前[官方模型发现文档](https://code.claude.com/docs/en/llm-gateway-protocol#model-discovery)也说明了此行为。
4. **任务跟踪工具受已知模型身份门控。** GPT 与普通陌生 ID 默认都缺少 TaskCreate 等四个工具，显式 `CLAUDE_CODE_ENABLE_TODO_TOOLS=1` 后真实本地 TaskCreate 成功。aisub2api 已有配置同步会补充该缺省开关。

因此，当前 CLI 接入的直接证据支持“存在名称过滤和能力门控，但未观察到 ChatGPT 专用的推理封禁或额外降档”。Desktop、Cowork 和 gateway 登录模式需分别验证，不能从 CLI 成功外推。

### 按两个目标判断

| 观察 | 行为判断 | 对正常使用的处理 |
| --- | --- | --- |
| 按 `claude/anthropic` 过滤模型发现；共享配置 schema 明确识别其他厂商 | 有意写入的身份筛选，属于直接的区别对待证据；恶意动机不能从条件判断本身证明 | CLI 使用显式 ID 或自定义选择入口；Desktop 最终阻断范围待验证 |
| 陌生模型默认不提供任务跟踪工具 | 已确认功能门控，影响不局限于 GPT | TODO_TOOLS 开关可恢复，aisub2api 已自动补缺省值 |
| WebFetch 仍依赖 Anthropic 域名检查，检查失败即停止抓取 | 已确认额外依赖；原生 Claude 对照同样失败，不能归为第三方专用惩罚 | 特定网络故障可通过 `skipWebFetchPreflight` 恢复，见下文实测 |
| 默认系统指令、thinking、effort | 本轮没有发现 GPT 相对普通陌生 ID 的特殊降质指令或参数降档 | 无证据要求伪装模型身份或强开更多能力 |
| 关闭遥测或非必要流量后部分功能消失 | 会影响远端 feature flags；服务依赖与本地工具门控需分开 | 不将这些变量默认加入第三方兼容配置 |

“没有推理封禁”不等于“没有区别对待”。本轮可以确认主动筛选及功能门槛的存在，但没有建立恶意降低第三方模型回答质量、故意制造延迟等结论；模拟响应也不能测量真实模型质量。

## 固定输入与覆盖范围

| 项目 | 证据 |
| --- | --- |
| 实际客户端 | `/home/quzhihao/.local/share/claude/versions/2.1.318`，本轮重新运行 `--version` 确认 |
| 二进制 SHA-256 | `708c77b204db30086ff6f1ed34577bda03e1be74ced80557cbf7b54249ebf0bd` |
| 模块清单来源 | 前轮固定 REA commit `71f680d7c1aa7a45ba0325fe87b104744b0c105f` 解析出的 Bun graph inventory |
| 本轮读取 | 校验二进制摘要，并逐一复核前轮 60 个模块的原始 contents 摘要后，按 inventory 的有界区间扫描全部 3,100 项 contents，共 57,455,202 字节；其中 2,857 项 loader 为 JS |
| AST 核验 | 独立 Babel parser 成功解析四个相关模块，定位下文函数和调用位置；没有执行提取出的模块 |
| 未覆盖 | Bun bytecode 反编译、独立 Desktop 安装包、私有服务端实现及所有分支的动态覆盖 |

扫描没有命中 `chatgpt` 字面量，但这不能证明没有相关检查：负向表达式中的 `gpt` 子串已经会匹配 `chatgpt`。其他 OpenAI 命中包括凭据检测、Codex 配置导入、Azure Foundry 域名和内置 Claude API skill 的适用范围，不应混同为推理封禁。

主要模块摘要：

| 模块 | SHA-256 |
| --- | --- |
| `chunk-1hnjmk23.js` | `e6834d3457c5e85387819ed87f5eb3cd65bf75d21f1b4c5b20ce2348f89fa2cf` |
| `chunk-exfmq47h.js` | `cd6e91b444dca42fe18f1e806dee19d53f60cc862d7d2e88c4329795768fd5e2` |
| `chunk-5xgkngpb.js` | `8b48a8c0f65827cc4e6a4c5493b98b26df81946b855ca19c45e15ed3eb884dff` |

## 新发现：严格厂商识别位于何处

以下 offset 是本版本原始 JS 的字符位置，只用于复核，不是稳定接口。

| 位置 | 已确认源码行为 |
| --- | --- |
| `chunk-1hnjmk23.js`，`f4`，89465 | 导出为 `isDefinitelyNotAnthropicModel`，以包含多厂商名称的正则判断模型 ID |
| 同模块，`Gf`，89584 | 导出为 `isLikelyAnthropicModelId`；先检查其他厂商负向表达式，再检查 Claude / Anthropic token 或 tier alias。因此同时含 `claude` 与 `gpt` 的 ID 仍会判为非 Anthropic |
| 同模块，`y4`，91344 | 导出为 `checkModel`，按 anthropic / bedrock / vertex / foundry / gateway / mantle 分支校验；provider 缺失或合法 tier alias 有单独放行规则 |
| 同模块，`Cue`，972964 | 导出为 `dropNonAnthropicModels`，遍历 `inferenceModels` 并调用上述检查；有有效条目剩余时才用过滤后的列表替换原列表 |
| 同模块，`$ue`，973397 | 单个模型的配置校验也调用 `checkModel`，生成非 Anthropic 名称的错误说明 |
| 同模块，`jge`，1208707；`mye`，1224790 | 配置验证、合并流程调用 `dropNonAnthropicModels`；相关结构包含 Desktop 的 `inferenceProvider` / `inferenceModels` |

**边界与源码推断：** 这份模块只导出 `startGateway`。对全部 JS contents 的引用搜索定位到 `chunk-nhaxap13.js` offset 48227：它在 `claude gateway --config` 子命令的 action 内动态导入此模块。当前证据将这段严格检查定位在内置 gateway 携带的配置处理代码，不能据此声称普通 `claude -p --model gpt-…` 会调用它。[官方 gateway 文档](https://code.claude.com/docs/en/claude-apps-gateway)确认 gateway 与 CLI 共用二进制；[配置文档](https://code.claude.com/docs/en/claude-apps-gateway-config#set-claude-desktop-settings-directly)说明 gateway 会使用 Desktop schema 校验相关配置。

也不能仅根据函数名宣称“任何 GPT 列表必被清空”：`Cue` 在过滤结果为空时保留原列表，只在仍有有效条目时替换。本轮没有运行 Desktop，也没有验证后续配置消费、启动或推理拒绝；具体界面效果仍待验证。

## 真实 CLI 对照实验

使用固定客户端、独立临时工作目录和 CLAUDE_CONFIG_DIR / XDG 目录、空 settings/MCP 来源、虚构 API key，以及只返回合成数据的回环 Messages fixture。模型响应由 fixture 决定，先请求 Read，再返回完成文本；未启动模型子 agent。模型名称仅作为测试标签，不代表真实账号可用性。

为了比较相同配置，所有场景使用 `ENABLE_TOOL_SEARCH=false`、`--effort high` 和相同系统提示。没有通过 aisub2api，也没有连接真实模型；这是客户端名称判断实验，不是上游能力测试。推理流量显式指向回环地址，关闭非必要流量和自动更新，但没有采用操作系统级断网隔离。

| 显式模型 ID | 退出码 / Messages 请求数 | Read | 默认四个任务跟踪工具 |
| --- | --- | --- | --- |
| `gpt-6.1-sol` | 0 / 2 | 成功 | 无 |
| `chatgpt/gpt-6.1-sol` | 0 / 2 | 成功 | 无 |
| `openai/gpt-6.1-sol` | 0 / 2 | 成功 | 无 |
| `codex` | 0 / 2 | 成功 | 无 |
| `gateway/model-x` | 0 / 2 | 成功 | 无 |
| `gateway/claude-gpt-6.1-sol` | 0 / 2 | 成功 | 无 |
| `claude-sonnet-4-6` | 0 / 2 | 成功 | 有 |

前六组的初始工具名称集合及以下字段逐项断言相同：

- `max_tokens = 32000`。
- `thinking = {type: adaptive, display: omitted}`。
- `output_config = {effort: high}`。
- `context_management` 含 `clear_thinking_20251015`，`keep: all`。

七组的 modelUsage 都报告 200000 contextWindow、32000 maxOutputTokens。这是隔离客户端默认值，不是实际模型容量，也不是 aisub2api 同步配置后的 128000 输出预算。Bash、Edit、WebSearch 等名称均出现在初始工具集合，但本轮只执行 Read，不声称这些工具的服务端能力均已验证。

陌生 ID 的请求含 mid-conversation system 消息，已识别的 Sonnet 4.6 请求没有；这符合[官方 gateway 契约](https://code.claude.com/docs/en/llm-gateway-protocol#feature-pass-through)对陌生模型的说明。这里的差异来自已知模型能力判断，GPT 与普通陌生 ID 之间没有差异。

### 任务跟踪工具的恢复

额外场景只为 `chatgpt/gpt-6.1-sol` 增加 `CLAUDE_CODE_ENABLE_TODO_TOOLS=1`：初始工具恢复 TaskCreate / TaskGet / TaskList / TaskUpdate；fixture 驱动真实本地 TaskCreate → Read → 最终文本，共三次 Messages 请求，退出 0，工具结果没有错误。

源码 `chunk-5xgkngpb.js` 的 `a8`（3324704）先检查会话状态、已知模型集合或 application inference profile，再读取 TODO_TOOLS 开关，没有 GPT 专用分支。aisub2api 的 `crates/aisub2api-cli/src/tui/mod.rs::apply_claude_code_env` 已在缺省情况下补充该开关，并保留用户显式值；本轮无需修改。

### 模型发现过滤与显式请求是两条路径

额外场景开启 `CLAUDE_CODE_ENABLE_GATEWAY_MODEL_DISCOVERY=1`，fixture 向真实 `GET /v1/models?limit=1000` 返回九个 ID。客户端 debug 记录缓存四个模型，缓存内容为：

- `gateway/claude-gpt-6.1-sol`
- `gateway/claude-model-x`
- `anthropic/model-x`
- `CLAUDE-CUSTOM`

被过滤的是 `gpt-6.1-sol`、`chatgpt/gpt-6.1-sol`、`openai/gpt-6.1-sol`、`codex`、`gateway/model-x`。同一进程显式选择被过滤的 `chatgpt/gpt-6.1-sol`，仍完成了两次 Messages 请求和 Read。

对应源码为 `chunk-exfmq47h.js::oWi`（414424），只按 ID 是否包含 `claude` / `anthropic` 做正向过滤。它与前述严格厂商识别不同：含 `claude-gpt` 的 ID 可以进入 discovery 缓存，但严格配置检查仍会因 `gpt` 判为非 Anthropic。不能把“进入列表”“通过配置校验”“允许发推理请求”当成同一个条件。

## 功能配置补查

### 默认系统提示是否给第三方增加特殊负面指令

前面的名称实验使用固定系统提示，因此不能回答默认系统指令是否不同。补查移除 `--system-prompt`，分别运行 GPT、普通陌生 ID、Sonnet 4.6、Sonnet 5.5 和 Opus 5.5，全部完成 Read。

对实际请求的顶层 system 和 messages 中的 system 文本一并比较：只归一化工作目录与模型身份后，GPT 与普通陌生 ID 完全相同，摘要均为 `08413009cc5c427d363bebc8c0b9a714c7ff6784e949ce12f96a1bda7d84fc52`。未归一化时，环境说明正确写出各自模型名称。

Sonnet 4.6 的顶层指令较长；GPT、普通陌生 ID 和 Sonnet 5.5 均使用较短的顶层指令并发送 mid-conversation system；Opus 5.5 也采用短指令结构。不能把相对旧 Claude 的指令长度变化直接当作第三方降质证据。本轮没有证明所有模型提示完全相同，也没有测量不同提示对真实模型质量的影响。

### 三类容易误用的兼容开关

相同 GPT ID、固定 `--effort high`，仅变更表中配置；每组都成功完成 Read，并核对实际请求而非只看退出码：

| 配置 | 实际结果 | 对功能异常的意义 |
| --- | --- | --- |
| `ANTHROPIC_DEFAULT_SONNET_MODEL` 指向 GPT，配套 `_SUPPORTED_CAPABILITIES=effort` | 仍发送 adaptive thinking、high effort 和 context management，与基线相同 | 普通自定义 `ANTHROPIC_BASE_URL` 接入不会消费这项声明，不能用它裁剪或恢复这里的能力 |
| `CLAUDE_CODE_DISABLE_ADAPTIVE_THINKING=1`，模型为 GPT | 仍发送 adaptive thinking | 不是陌生模型的通用降级开关 |
| 同一 adaptive 开关，模型为 Sonnet 4.6 | 改为 `thinking.type=enabled`，带固定 budget | 正向对照确认开关确实生效，但有模型范围限制 |
| `CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS=1`，模型为 GPT | 删除 context management，仍发送 adaptive thinking 和 high effort；部分 beta header 仍保留 | 不能把它当成删除所有 beta、thinking 和 effort 的总开关 |

源码依据：`chunk-exfmq47h.js::r$`（489125）在 `Iu()` 为真时直接跳过 capability override；`chunk-5m8x0kb6.js::Iu`（32450）包含 firstParty 和 gateway 等接入类型，普通 `ANTHROPIC_BASE_URL` 仍属于 firstParty 协议路径。`chunk-exfmq47h.js::EHe`（492323）仅在 canonical ID 包含 `opus-4-6` 或 `sonnet-4-6` 时使用该 adaptive 禁用变量。对应[官方 capability 声明范围](https://code.claude.com/docs/en/model-config#customize-pinned-model-display-and-capabilities)与[beta 禁用说明](https://code.claude.com/docs/en/llm-gateway-protocol#disable-pre-release-capabilities)需结合具体接入方式阅读。

### WebFetch：额外服务不可用时的明确配置解法

`chunk-5xgkngpb.js::Wfr`（2230321）使用固定地址 `https://api.anthropic.com/api/web/domain_info` 检查目标 hostname。即使模型接口已经指向自定义地址，该检查仍会访问 Anthropic。检查失败时抛出 DomainCheckFailedError，目标网页不会被请求；`skipWebFetchPreflight` 在抓取入口决定是否执行检查（2237119）。

补查使用本地 HTTPS 模拟代理和仅对隔离子进程生效的临时证书，所有 HTTPS 目标由代理返回合成响应，代理不转发外网。分别控制检查接口返回 503 或 `can_fetch=true`，并记录检查接口、目标页面及模型请求：

| 模型 / 配置 | 域名检查 | 是否请求目标页面 | WebFetch 结果 |
| --- | --- | --- | --- |
| GPT，缺省配置 | 模拟 503 | 否 | tool_result 为错误 |
| GPT，`skipWebFetchPreflight=true` | 未请求 | 是 | 成功 |
| GPT，缺省配置 | 模拟允许 | 是 | 成功 |
| Sonnet 4.6，缺省配置 | 模拟 503 | 否 | 与 GPT 相同的检查错误 |

成功组还断言合成页面 marker 到达 WebFetch 后续模型处理请求；模型摘要由 fixture 决定，不代表真实网页或真实模型质量。失败组客户端仍退出 0，因为 fixture 随后返回结束文本；验证以网络记录和 `tool_result.is_error` 为准，不能以进程成功掩盖工具失败。

遇到该检查不可用的错误时，可合并以下顶层设置；它不是 env 项：

```json
{
  "skipWebFetchPreflight": true
}
```

此项跳过 Anthropic 的域名黑名单检查，抓取仍遵循 WebFetch 权限规则；不能替代目标站点联网、模型摘要能力或服务端 WebSearch。它不是第三方模型必需的通用设置，所以本轮没有全局自动启用。[官方设置说明](https://code.claude.com/docs/en/settings-reference#skipwebfetchpreflight)已明确用途与作用范围。[官方数据使用说明](https://code.claude.com/docs/en/data-usage#default-behaviors-by-api-provider)确认 `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC` 不会关闭这个检查，实测也保留了该变量并观察到检查请求。

[官方仓库 issue #24921](https://github.com/anthropics/claude-code/issues/24921)报告过 2.1.39 macOS 自定义地址下的同类症状，当前状态为 Closed as not planned。本轮独立复现的是 2.1.318 Linux 的检查依赖及配置恢复，不据旧 issue 的描述推断当前权限弹窗或关闭原因。

### 关闭额外流量也可能让功能消失

当前[官方 feature flag 说明](https://code.claude.com/docs/en/env-vars#features-that-need-feature-flag-fetching)明确列出：关闭 telemetry / growthbook / nonessential traffic 会停止远端 feature flag 获取；第三方 provider 和 Claude apps gateway 也有相应的获取限制。部分 Remote Control、import、advisor、MCP tasks 等能力依赖这一路径。这类能力不能承诺仅靠一个本地开关恢复，也不能把主动关闭获取后的缺失直接当成模型能力问题。

本轮隔离实验一直关闭额外流量和 telemetry，足以比较相同条件下的本地行为，不构成远端功能开关的生产覆盖；没有为了实验开启实际遥测或云服务。

## 对 aisub2api 的影响与未验证项

| 项目 | 分类 | 当前动作或边界 |
| --- | --- | --- |
| CLI 显式 GPT 名称能否调用模型 | 已验证客户端路径 | 使用真实完整 ID 或现有角色映射；没有证据要求修改模型 ID |
| 自动发现不展示 GPT | 已复现 | 完整 ID、现有角色配置或自定义 picker 属于独立选择入口；本轮没有做交互 picker UI 验收 |
| GPT 缺少本地任务跟踪 | 已覆盖 | 现有配置同步补缺省 TODO_TOOLS；不覆盖用户禁用值 |
| GPT 比普通第三方 ID 少发 thinking / effort | 本轮未复现 | 两者发送字段相同；上游协议转换与真实模型接受度另行判断 |
| GPT 相对普通陌生 ID 多出负面系统指令 | 本轮未复现 | 默认系统指令归一化目录、模型身份后相同；包含 mid-conversation system 的比较 |
| WebFetch 因 Anthropic 域名检查不可用而失败 | 已复现并验证配置恢复 | 条件使用顶层 `skipWebFetchPreflight=true`；新增使用指南，不全局改变域名检查 |
| 普通自定义地址下 capability 声明无效、adaptive 禁用开关无效 | 已复现 | 记录准确作用范围，不把它们加入通用兼容配置 |
| 严格厂商识别对 Desktop / Cowork 的最终效果 | 待验证 | 已定位代码，不宣称完成 Desktop 动态验证，也不自动引入名称混淆 |
| skill / agent frontmatter 的模型限制 | 待验证 | [clodex 作者文档](https://github.com/bman654/clodex#readme)提到另一组名称限制和 masking；这是后续线索，本轮没有复现，不能当作本机已确认故障 |
| ChatGPT OAuth backend 的拒绝、额度或能力 | 不属于本轮验证 | 本轮没有访问真实上游，不能将其错误归因于 Claude 客户端名称限制 |

## 已落地改进

- `aisub2api config claude --webfetch-preflight skip|check`：显式修改 Claude 用户配置中的顶层布尔值，保留其余字段；带选项时只改 WebFetch，可在未配置代理时使用。常规同步继续保留用户选择，缺省不自动跳过检查。
- `aisub2api config claude`：复用现有同步路径，提供独立的模型、角色和缺省兼容配置同步入口；要求 provider / 主模型配置就绪，不启动服务。
- `aisub2api config validate`：增加只读 Claude 用户设置诊断，指出路由与凭据不一致、任务工具、工具搜索、WebFetch 值类型 / 放置位置及已知无效开关。输出不打印凭据和设置原值，不声称覆盖项目、组织策略、shell / CLI 或运行中的配置；提醒不导致失败，损坏的 JSON / 结构 / WebFetch 布尔类型导致失败。
- `scripts/claude-compat-smoke.py`：将关键验收纳入仓库，使用真实客户端、生成的配置及本地 Messages / MCP / HTTPS fixture。覆盖 TaskCreate、Read、完整发现后的工具 schema 与 MCP 执行，以及 WebFetch 错误、跳过和恢复检查；默认不加载现有用户 / 项目设置或 MCP。

本轮实现集中在 CLI、共享 Claude 配置读写与验收脚本，没有修改 provider 行为、个人设置或运行中的服务。使用方式见[排障指南](../guides/claude-code-third-party.md)。

## 验证与本地产物

九个隔离场景合计 19 次 Messages 请求；所有客户端退出码均为 0。断言覆盖相同陌生模型请求字段和工具名称、Read 成功、任务跟踪恢复、无工具结果错误，以及九进四的 discovery 缓存结果；四个目标模块 AST 解析通过。

功能补查新增 14 个隔离场景、30 次 Messages 请求，覆盖默认提示、开关作用范围以及 WebFetch 失败和恢复。四个 WebFetch 场景中两组工具失败是预期对照，全部满足对应断言。累计 23 个场景、49 次 Messages 请求；不将其当作真实模型可靠性统计。

临时目录 `/tmp/aisub2api-cc-chatgpt-restrictions-20261106` 保存：

- `full-readable-scan.json`、`source-matches/`：输入摘要、区间、命中与模块摘要；提取内容不进入仓库。
- `inspect_targets.mjs`、`ast-targets.json`：定向 AST 定位。
- `probe_model_names.py`、`model-names-summary.jsonl`：七组名称对照和任务开关实验。
- `probe_discovery.py`、`discovery-summary.jsonl`：真实客户端模型发现实验。
- `verification.json`：逐项断言结果及请求字段投影；各场景目录保存合成请求、客户端输出和缓存。
- `functional-audit/`：功能补查的脚本、summary、模拟网络记录及 `verify.py` / `verification.json`；其中临时证书仅供本轮本地 fixture 使用，没有安装到系统证书库。

临时文件可能被清理；本文表格和固定输入摘要记录调研结论，仓库验收脚本可在后续版本复跑。实现验收：CLI all-targets 共 64 项 Rust 测试通过，Clippy（warnings denied）、fmt 和构建通过；真实 Claude Code 的五个仓库脚本场景共 14 次 Messages 请求通过，含预期 WebFetch 错误及恢复。文档检查格式、相对链接和 diff。验收结果不外推为真实模型或生产链路可靠性。
