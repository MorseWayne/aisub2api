# 运行日志、metrics 与 Claude Code 会话对照

日期：2026-11-05。代码基线：`1b2ed2c`（v6.0.4）。本次只读观察实际工作实例和客户端记录，随后在源码与隔离测试中修复两个诊断缺口；未重启或热更新工作实例。

## 最重要的版本事实

宿主进程实际运行 **6.0.2**，安装路径中的文件已经是 **6.0.4**。宿主 `/proc/<pid>/exe` 指向已删除的旧安装文件，重新执行该映像的 `--version` 返回 6.0.2；磁盘安装文件返回 6.0.4。

- 运行映像 SHA-256：`4eaac69637624bc87a8ea66a27bf991835ec785566c58fd15f46d42991a9791a`。
- 已安装文件 SHA-256：`32a662389e9e9c959fee941dba140cd14485fdad147c1f995ead6ee1d4321ca7`。

此前升级时只核对安装文件，并因沙箱 PID 视图看不到宿主进程而误判没有运行实例，漏了运行映像验证。本次使用获准的宿主只读检查纠正。不能把本次生产指标当作 6.0.4 的效果验证，也不能凭沙箱中 PID 不存在就判断服务停止。

当前任务仍在工作，因此本轮没有进行进程切换。之后需要在安全空闲时停止旧进程，再从新版二进制启动；当前 `server restart` 使用配置重载信号，并不能替换已加载的程序映像。

## 观察窗口与来源

统一统计窗口为 UTC `[2026-11-05 03:45:00, 04:10:00)`，即北京时间 11:45–12:10。服务和客户端都在继续写日志，所以采用固定窗口而不是混用不同查询时刻的计数。

- 代理主日志：现有日志目录中的 `aisub2api.log`。另一个控制台落盘文件包含重复事件，不重复计数。
- SQLite：`usage_events` 与 `request_observability_events`，使用只读连接和短事务，不改数据库。
- `/admin/metrics`：使用配置的 Bearer 管理认证读取；首次以 x-api-key 探测得到的 401 是本次管理探测，不是用户推理故障。
- Claude：当前项目两个 JSONL 会话的请求参数、时间、版本、message identity、usage 和工具事件。debug/latest 是旧的悬空链接，没有把它当作当前 debug 日志。

同一 assistant message.id 可能按 thinking/tool 等分成多条记录；usage 按 message identity 去重，内容类型和工具名合并，避免把重复 usage 相加。

窗口内读取到 69 条 Claude 主线程 api-request，均为 2.1.318、effort xhigh。按模型和近似请求开始时间（代理记录结束时间减总耗时，容差 1.2 秒）唯一关联 68 条已完成请求。这个关联没有共享 request ID 的保证，窗口边界未匹配的请求不视为丢失。

两个 SQLite 表的 id 没有跨表外键语义。本次对照还核对 provider、model、initiator、created_at 及耗时字段，不把 id 恰好一致当作长期关联契约。

## 运行结果

| 指标 | 固定窗口结果 | 含义 |
| --- | --- | --- |
| 完成的代理请求 | 104，代理记录错误 0 | 不能将所有等待都判断为失败 |
| transport | 104 条均为 SSE | 当前没有 WebSocket continuation 节省 |
| 合成 conversation identity | 104 条均为 true | 旧版本还没有本会话加入的 Claude JSON user_id 解析 |
| 总耗时 | P50 约 6.1 秒，P95 约 19.8 秒 | 有一个大输出显著拉高均值 |
| 首个事件 | P50 约 1.75 秒，P95 约 3.64 秒 | 不等于首个可见文本 token |
| provider setup | P95 2 ms，最大 3 ms | 本地 provider 选择/准备不是本窗口的主要耗时 |
| 最大单事件间隔 | 窗口最大约 14.6 秒 | 没有由此证明 300 秒 idle timeout 被触发 |
| 上游报告的 token | 非缓存输入 521,957，缓存读 7,093,888，输出 42,586 | 按当前转换口径，缓存读约占输入 token 的 93.1%；不是逐请求命中率或费用节省比例 |

`upstream_connect_ms` 包含上游请求发出到响应头返回的等待，不能直接解释成纯 TCP/TLS 时间。未单独测量 DNS、TLS 和模型排队，不据此调整连接池或 TLS 策略。

### WebSocket 为何没有继续使用

代理日志记录了前一天 UTC 09:35:51 的一次 `tls handshake eof`，随后禁用了该 provider 实例的 WebSocket，退回 SSE。数据库最后一个 WebSocket 完成记录为 UTC 09:36:13，随后保持 SSE。

这是现有实例内单向 fallback 的行为，不是本轮发现的重复重连故障。配置仍为 transport auto。加载新实例后可以重新验证 WebSocket 是否可用，以及 6.0.3 起的连接池和 6.0.4 会话身份是否生效；不能保证外部握手一定成功，也不为这次观察重新加周期性探测状态机。

完整发送请求体不等于没有缓存。日志还记录了 zstd 压缩，`request_body_bytes` 等逻辑字节不能直接当作网络流量；累计压缩体也不等于 TCP 总量或服务端计费。

### 慢请求发生在哪一层

| 代理完成时间 UTC | 请求总耗时 | Claude/usage 对照 | 判断边界 |
| --- | --- | --- | --- |
| 03:54:10 | 250.869 秒 | Write，输出 13,796 tokens，其中 reasoning 1,423；最大事件间隔 4.990 秒 | 持续的大输出，不是这次流里 250 秒完全无事件 |
| 04:07:43 | 56.021 秒 | Read 前一轮生成，输出 2,852，其中 reasoning 2,793 | 大部分输出为推理 |
| 04:05:43 | 51.706 秒 | Read 前一轮生成，输出 2,640，其中 reasoning 2,587 | 同样以推理为主 |

Claude 还记录了 Bash 约 134 秒、Write 约 51 秒的工具调用至返回间隔。它们可能包含执行、权限交互或排队，日志不足以全部分离；不能归入代理推理 HTTP 耗时，也不能据此断言文件系统慢。

窗口内有一次 Artifact 工具结果标记为 error。这属于客户端工具结果层，未伴随代理推理失败；没有基于这一个标记改写代理的协议或工具参数。

## 新增的诊断修复

### 流式 latency 少计

源码确认 `stream_leader_response` 丢弃了请求 start，仅把 task_start 之后的流传输时间写入 usage 和平均耗时。non-stream 路径则用请求总耗时，口径不一致。

窗口内 76 个 stream 请求的 usage latency 全等于 stream_duration，平均 10,831.88 ms；对应 total latency 平均 12,875.86 ms，少计平均 2,043.97 ms。28 个 non-stream 请求两者一致。

改动：平均/持久化 latency 改为请求 start 到完成的时间；stream_duration 保留原流传输阶段时间。没有更改请求执行、重试、模型配置或历史数据库记录。更新后数值可能上升，是统计口径修正，不能宣称实际请求更快。

新增回归在不 sleep 的条件下模拟流开始前已花费 500 ms。修改前失败，平均 latency 只有 1 ms；修改后总耗时包含该时间，同时保留独立 stream duration。

### 暴露运行中服务版本

`/admin/metrics` 新增只读 `server_version`，权威来源为 server crate 编译时的 package version，便于远程诊断安装文件与工作实例不同的情况。它是 additive 字段，没有改认证、既有 JSON 字段或数据库 schema。

这只是 package version，不证明相同版本字符串下的任意开发构建字节一致。精确比对仍使用映像摘要。当前 6.0.2 工作实例不会自动出现此字段，需后续部署本轮代码才可用。

## 验证与优先级

- `cargo test --locked -p aisub2api-server -- --quiet`：138 项通过（unit 116、integration 22）。
- `cargo clippy --locked -p aisub2api-server --all-targets -- -D warnings`、格式检查、diff 检查及 CLI debug 构建通过。
- 用临时 XDG/Claude 配置和随机回环端口启动隔离实例，管理 GET 返回的 server_version 与该二进制版本一致；未向真实模型发送请求。
- 本次没有停止、重载、发布或替换工作实例。

后续优先顺序：安全空闲时先加载已安装的新版本，再在一致时间窗口比较 runtime 版本、真实 conversation identity、transport、continuation 和完整请求耗时。任务/effort 的分层对照需要用户选择质量与预算目标；不从代理层静默降低当前 xhigh。没有数据支持本轮再增加缓存、索引、自动重试或历史裁剪。

私有汇总保存在 `/tmp/aisub2api-live-20261105`，未加入仓库。报告仅保留诊断字段和聚合信息，不包含对话正文、工具输入内容、凭据或账号余额。
