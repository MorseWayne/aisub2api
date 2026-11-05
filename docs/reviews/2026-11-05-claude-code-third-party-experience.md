# Claude Code 第三方 provider 限制与使用体验优化

日期：2026-11-05。基线：`72b19f0`。范围：当前 Linux Claude Code 2.1.318，aisub2api 的客户端配置同步、公开 Messages 接口和模型选择。优先修复已复现问题，其他限制给出适用条件与规避方式。操作指南见[第三方 provider 使用指南](../guides/claude-code-third-party.md)。

## 固定证据与日志

- Claude 二进制：`/home/quzhihao/.local/share/claude/versions/2.1.318`，SHA-256 `708c77b204db30086ff6f1ed34577bda03e1be74ced80557cbf7b54249ebf0bd`。沿用已核对的定向可读模块；没有反编译字节码，也不使用未经核验的泄漏源码。
- `chunk-exfmq47h.js` 摘要 `cd6e91b444dca42fe18f1e806dee19d53f60cc862d7d2e88c4329795768fd5e2`：上下文解析 AA/xA、输出限制 H_e/BNn、模型 catalog/identity 分支。
- `chunk-5xgkngpb.js` 摘要 `8b48a8c0f65827cc4e6a4c5493b98b26df81946b855ca19c45e15ed3eb884dff`：工具搜索 kJr/Qet、细粒度工具流式 rZt。固定 symbol/offset 只是本版本定位信息，不是稳定契约。
- 只读本机 Claude JSONL 的 UTC `[04:30,07:00)` 窗口：185 条主线程 api-request 均为 max_tokens 128000、effort xhigh；观察到 Bash/Read/Edit 等实际工具调用，没有据此把所有长等待归为 provider 限制。API 请求日志可能为每次重试记录一条，不当作独立用户任务数。
- 当时用户的主模型和三个角色已全部显式配置，工具搜索为 true；未配置输出预算覆盖、精确窗口覆盖、fine-grained 开关或 modelPicker。日志没有 TaskCreate 调用，不能仅据此断言工具未注册；工具可用性通过下面的独立请求实验确认。没有更改这些个人配置。

## 多来源对照与处理

| 证据 / 限制 | 本项目验证 | 分类与最小动作 |
| --- | --- | --- |
| [官方 gateway guide](https://code.claude.com/docs/en/llm-gateway-protocol)：主模型与背景角色解析不同 | 只配主模型的真实 CC 通过本工具调用 haiku/sonnet/opus，发出了上游不提供的原生 Claude ID | 已复现需修复：非 Anthropic 主 provider 的未配置角色继承主模型，显式角色仍优先；原生 Anthropic 保留默认解析 |
| 配置同步固定写入输出上限 | 用户已有 8192 被改成 128000，模拟上游实际收到 128000 | 已复现需修复：改为只补缺省值；不静默降低上限或 effort |
| [#104309](https://github.com/anthropics/claude-code/issues/104309)，Open，更新 2026-10-25，报告 macOS 2.1.306–2.1.310 的核心工具丢失 | 2.1.318 Linux 主线程 true/auto/false 均保留 Read/Bash，并实际完成 Read；未测试真实 subagent | 该失败场景在本轮主线程未复现；新配置默认 auto，保留用户已有开关，不全局禁用工具搜索 |
| [#65966](https://github.com/anthropics/claude-code/issues/65966)，Closed，基于 2.1.112–2.1.167 的旧门控调研 | 当前裸陌生 ID 可直接通过 MAX_CONTEXT 声明 500K，无需 DISABLE_COMPACT；[1m] 和已识别 Claude ID 另有规则 | 旧结论部分已过时；给出实测矩阵，避免通过关闭压缩来盲目扩窗。该 issue 自己撤回的“忽略缓存 token”指控不作为优化依据 |
| [#88345](https://github.com/anthropics/claude-code/issues/88345)，Closed，含 Desktop 的第三方 gateway 配置与窗口反馈 | 本轮只测 CLI；没有重现 Desktop host 配置优先级或 URL 路径裁剪 | Desktop 专属部分待验证，不把改 CLI settings 当作 Desktop 的通用解决方案 |
| [官方环境变量](https://code.claude.com/docs/en/env-vars)：任务工具存在模型门控，ENABLE_TASKS 与 ENABLE_TODO_TOOLS 含义不同 | 陌生模型默认没有 TaskCreate；TODO_TOOLS=1 恢复工具，并能在 ToolSearch 发现后实际创建本地任务。ENABLE_TASKS=1 单独无效 | 配置同步自动补充 TODO_TOOLS=1；已有禁用值及 legacy TodoWrite 选择保留 |
| 旧报告称自定义地址不能开启 fine-grained tool streaming | 当前显式开关产生 eager_input_streaming，完整 Read 调用成功 | 已验证客户端可发字段；与上游是否支持、是否更快分开，不全局强开 native beta |
| [当前模型配置](https://code.claude.com/docs/en/model-config)、[gateway discovery](https://code.claude.com/docs/en/llm-gateway-protocol#model-discovery) | 模型选择入口与能力/窗口声明不是同一契约；当前 list_models 另有内部 capabilities，但不能假定 CC 自动消费 | 提供完整 ID / 自定义 picker 配置途径，不通过伪装 Claude 模型名或新增无效 metadata 绕过客户端过滤；交互 picker 未做 UI 验收 |
| [官方功能矩阵](https://code.claude.com/docs/en/feature-availability)：本地能力、服务端能力与订阅服务分开 | 本地 Read 和任务操作已验证；未调用真实搜索服务、云端 session 或 Remote Control | MCP/本地工具可作明确替代；不承诺代理能补齐订阅服务，也不伪造服务端功能成功 |

同类实现另读取了 [claude-code-router environment.ts](https://github.com/musistudio/claude-code-router/blob/057abaa1f95519ab8faf999e1ccef2b80ccdeb9e/packages/core/src/agents/claude-code/environment.ts)，固定 commit `057abaa1f95519ab8faf999e1ccef2b80ccdeb9e`。它分别管理主模型与 Fable/Opus/Sonnet/Haiku 的 env，并清理旧变量，说明只设置主模型不足以表达全部角色。本项目复用现有三个角色字段，不引入其额外路由或 Fable 配置，也不照搬未经本地证据支持的其他环境改写。

## 配置同步修复

修改集中在 CLI 的客户端配置生成和保存流程，覆盖启动服务、TUI 保存以及 CLI 配置变更：

1. 已有 `CLAUDE_CODE_MAX_OUTPUT_TOKENS` 不再被重置。未配置时维持 128000 默认值，客户端和上游仍可能有自己的限制。
2. 主 provider 的类型以配置中的 provider_type 为权威，缺配置时才按 ID 推导。类型不是 Anthropic 时，空白或缺省的 Haiku/Sonnet/Opus 使用投影后的主模型；直接 Anthropic 的未配置角色仍清除旧覆盖，让原生默认值生效。
3. 自动补充 TODO_TOOLS=1，让陌生模型也能使用本地任务跟踪；新配置的工具搜索使用 auto，而不是无条件延迟加载。已有显式开关、legacy TodoWrite、fine-grained 和 picker 保留。
4. CLI 完成 provider 配置、切换或导入后自动同步客户端设置，与 TUI 保存/启动服务保持一致；未完成 provider 选择时跳过，损坏的 Claude JSON 不覆盖并报告同步失败。
5. 显式角色模型、reasoning 模型保持原控制语义；没有更改模型权限、登录状态、provider 认证、上游请求转换或正在运行的服务。上下文继续使用现有自动策略，不新增缺乏 per-model 作用域的全局窗口覆盖。

继承主模型是可用性缺省，不是质量分级。希望角色使用不同模型、尤其希望背景调用更便宜时，仍应显式配置角色。用户本机已有的四个显式模型不会因这次缺省修复改路由。

## 真实客户端实验

全部使用固定客户端、临时 CLAUDE_CONFIG_DIR/XDG 目录、空 settings/MCP 来源或明确的隔离用户设置、假凭据及随机回环端口。模拟响应只驱动协议，不证明真实模型能按同样方式自主选择工具，也不证明成本或质量改善。没有启动子 agent 或访问真实推理服务。

### 主模型、角色与输出预算

`Claude Code → 隔离 aisub2api → 只提供 gpt-5.5 的模拟 Responses 服务`。配置只设置 `model.default=mock/gpt-5.5`，客户端原有上限为 8192：

| 选择 | 修改前 | 修改后 |
| --- | --- | --- |
| default | 成功，但 8192 被改成 128000 | 成功，上游收到 gpt-5.5 / 8192 |
| haiku | 请求 claude-haiku-5-5，两次上游 model_not_found，退出 1 | 请求 gpt-5.5 / 8192，退出 0 |
| sonnet | 请求 claude-sonnet-5-5，两次上游 model_not_found，退出 1 | 请求 gpt-5.5 / 8192，退出 0 |
| opus | 请求 claude-opus-5-5，两次上游 model_not_found，退出 1 | 请求 gpt-5.5 / 8192，退出 0 |

这是角色选择实验，没有直接跑背景摘要、Explore 或并行 subagent；对这些调用方的收益属于复用相同角色解析的源码推断。

### 工具与窗口

- `ENABLE_TOOL_SEARCH=true/auto/false` 三组均完成两轮 Read。true 的初始工具集合较小，但仍有 Read/Bash；auto/false 在本样本中直接加载更多定义。未测大型 MCP 集的模型质量或 token 收益。
- fine-grained 显式开启时，工具定义出现 eager_input_streaming，Read 正常完成；不开启时没有该字段。没有据此改默认值或声称 OpenAI 上游生成更快。
- 陌生模型默认缺少 TaskCreate；TODO_TOOLS=1 且搜索关闭时直接提供四个 Task 工具。搜索开启时，先 select:TaskCreate，下一轮出现该 schema，之后真实本地 TaskCreate 成功并完成 Read，共四轮；初始缺少 schema 本身不等于功能被封禁。
- 五组 contextWindow 结果：裸陌生 ID 200000；声明 500000 后为 500000；[1m]+声明仍为 1000000；再禁用 1M 后为 500000；已识别 Sonnet 4.6 仅声明 500000 仍为 200000。数据来自 CC 的 modelUsage，不是上游容量测量；没有用长会话触发真实自动压缩。

上下文源码在固定 `chunk-exfmq47h.js` 的 `AA`（字符 offset 486287）区分已知 catalog、[1m] 和未识别 ID；`CA` 判断何时接受 env 覆盖。对应真实实验确认当前路径，不把历史“必须关压缩”的建议套到所有模型。

### 无需手工开关的自动配置验收

通过新构建的 `config import` 生成隔离 Claude settings，然后让真实 CC 读取生成的用户设置，直连本地 Messages fixture。没有从 shell 或 `--settings` 手工注入任务/搜索兼容开关，没有启动代理工作实例：

- 新配置自动得到 TODO_TOOLS=1、ENABLE_TOOL_SEARCH=auto。陌生模型初始工具集合直接含 TaskCreate，之后 TaskCreate → Read → 最终文本，共三轮，全部成功。在此小工具集样本中省去了一次发现 TaskCreate 的 ToolSearch 往返，不将其推广为所有任务的延迟或成本改善。
- 预置 TODO_TOOLS=0、搜索 false、输出 8192 和 fine-grained 0 的用户设置后再次导入，所有显式选择保留。CC 不提供任务跟踪工具，但 Read 正常完成，两轮结束。
- 这次通过 CLI 的正常保存路径验收自动配置，无需用户再编辑 CC 环境变量。之前手动指定单个开关的实验仍用于解释原始门控和覆盖行为。

## 验证与产物

- 修改前配置同步回归失败，确认 32000 被覆盖为 128000，且未配置角色没有安全映射。
- 修改后 `cargo test --locked -p aisub2api-cli --all-targets --quiet`：51 项（46 unit + 5 integration），覆盖原生 provider 类型、显式角色、缺省策略、既有预算/picker，以及真实 CLI 导入/切换自动同步、用户覆盖、损坏 JSON 和未完成 provider 配置的保护。
- CLI Clippy（warnings denied）、fmt/diff 检查和 debug 构建通过；实际客户端经新构建的四个角色场景均通过；自动配置新增了不手动注入兼容开关的验收，以及保留显式禁用的对照。
- 私有临时目录 `/tmp/aisub2api-cc-third-party-20261105` 保存脱离生产配置的实验、summary、定向源码命中与社区 revision；此前可读模块仍在 `/tmp/aisub2api-rea-20261105/source-selected`。这些临时文件不加入仓库。

本文记录分析与隔离验证阶段的结果，当时没有发布或更新工作实例。部署包含这些改动的构建并同步客户端设置后才会应用新的缺省策略；重启 CC 后应重新核对实际请求模型、输出上限和工具选择。
