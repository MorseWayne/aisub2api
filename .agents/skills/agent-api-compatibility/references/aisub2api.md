# aisub2api 项目上下文

仅当目标仓库是 aisub2api 时读取。以下路径相对于目标仓库根目录，先验证实际 checkout 与当前代码；不要把它们解析成全局 skill 的相邻目录。

## 请求路径

- `crates/aisub2api-core/src/types.rs`：canonical Messages、Content、Tool、模型能力及序列化。检查未知字段、cache_control、thinking 选项和 inline system 消息是否保留语义。
- `crates/aisub2api-server/src/routes.rs`：鉴权、model/provider 解析、能力检查、inflight 去重、流式转发、错误映射、日志与指标。
- `crates/aisub2api-server/src/downstream.rs`：Chat Completions/Responses 编码及 HTTP/SSE 错误契约。
- `crates/aisub2api-providers/src/anthropic.rs`、`copilot/`：原生 Messages 路径及 provider 特有的头、参数和净化。
- `crates/aisub2api-providers/src/openai.rs`、`openai_compat.rs`、`chat_completions.rs`、`responses.rs`：协议转换、effort、工具引用、分段参数与终结处理。
- `crates/aisub2api-providers/src/chatgpt.rs`、`chatgpt/responses.rs`：ChatGPT backend、conversation identity、continuation 和 prompt-cache 作用域。
- `crates/aisub2api-providers/src/sse.rs`：SSE framing 与共用原生 Messages 接收。确认当前版本，不假设历史修复已提交或安装。

OpenAI 公共 API、ChatGPT/Codex backend 和 Copilot 的行为不能直接互相替代。客户端协议、provider 上游端点、账号能力和代理对外声明分别审查。

## 已有证据的入口

在 `docs/reviews/` 查找与目标版本对应的记录。这些是历史实验，不是永久契约：

- `2026-11-05-claude-code-rea-alignment.md`：协议头/请求去重、effort 与结构化输出、session identity、thinking.display；含固定产物与 REA 提取边界。
- `2026-11-05-claude-code-tool-search-recovery.md`：受控 MCP 和 422→502 对恢复的影响。
- `2026-11-05-claude-code-openai-effort.md`：输入设置、实际发送与日志之间的差异，以及现有优先级。
- `2026-11-05-claude-code-stream-completion.md`：成功终结、截断、空响应与完成后的 HTTP 关闭。
- `2026-11-05-community-compatibility-evidence.md`：同类 issue/PR、版本差异与已覆盖场景。

不要依赖某次会话的 `/tmp` 文件继续存在；先检查，缺失时按记录重新准备最小 fixture。正式回归优先使用仓库测试及脱敏 fixtures。

## 验证与相邻 skill

先运行与改动匹配的包或测试；共用协议变更再覆盖 provider/server 受影响路径。常用命令：

```sh
cargo fmt --all --check
cargo test --locked -p aisub2api-providers -p aisub2api-server -- --quiet
cargo clippy --locked --workspace --all-targets -- -D warnings
git diff --check
```

网络相关回归常需绑定回环地址，按实际工具权限执行。本地 fixture 通过不等于真实 OAuth backend 验证通过。

若用户明确要求同步 openai/codex 或分析其 revision delta，可按项目已有的 `codex-upstream-alignment` skill 处理该范围。本 skill 不自动调用它或继承其强制同步动作；一般适配研究不需要 reset/clean 参考仓库。
