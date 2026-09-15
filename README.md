# svn-ai

`svn-ai` 是一个面向 Claude Code 与 Subversion（SVN）的离线优先代码归因工具。它通过 Claude Code Hook 记录 AI 对文件的修改，并在代码提交 SVN 后统计最终入库代码中的 AI 代码、人工代码和歧义代码行数。

## 当前能力

- 支持 Windows 和 Linux 命令行客户端；
- 支持 Claude Code 的 `Edit`、`Write` 工具；
- 自动发现文件所在的 SVN 工作副本；
- 使用本地 SQLite 保存采集状态；
- 最终归因事件只保存带密钥的代码行指纹和仓库相对路径；
- 可通过 `svnlook` 对已提交的 SVN 修订执行本地统计；
- 支持中文 Windows 控制台编码、中文提交人和中文 SVN 路径。

当前尚未实现：

- Claude Code `Bash` 命令产生的文件变化采集；
- 客户端事件自动上传；
- VisualSVN 服务端 Hook Agent；
- Linux 中央统计服务和管理后台。

因此，本阶段适合验证采集和归因算法。`svn-ai stats` 仍需在能够访问 SVN 仓库物理目录的环境中执行。

## 工作原理

1. Claude Code 调用 `Edit` 或 `Write` 前，`PreToolUse` Hook 保存短期文件快照；
2. 工具执行成功后，`PostToolUse` Hook 计算新增和修改代码行的带密钥指纹；
3. 工具执行失败时，`PostToolUseFailure` Hook 删除短期快照；
4. 代码提交 SVN 后，`svnlook` 读取对应修订；
5. `svn-ai` 将提交中的新增行与本地 AI 指纹进行一对一匹配并输出统计结果。

Hook 发生异常时只记录本地诊断日志，不会阻断 Claude Code 操作。

## 编译

要求：Rust 1.85.1，包含 Cargo、Rustfmt 和 Clippy。

```powershell
cargo build -p svn-ai --release
```

Windows 可执行文件位于：

```text
target\release\svn-ai.exe
```

Linux 可执行文件位于：

```text
target/release/svn-ai
```

## Windows 手工验证

以下流程会在本机创建一个独立测试仓库，不连接也不修改正式 SVN。

### 1. 准备 SVN 命令行工具

需要同时具备：

```powershell
svn --version --quiet
svnadmin --version --quiet
svnlook --version --quiet
```

Windows 可以使用 [SlikSVN](https://sliksvn.com/download/) 或安装了命令行组件的 TortoiseSVN。以下示例假设：

```powershell
$svnAi = "D:\path\to\svn-ai.exe"
$svnExe = "D:\Tools\SlikSVN\Portable\PFiles\bin\svn.exe"
$svnlookExe = "D:\Tools\SlikSVN\Portable\PFiles\bin\svnlook.exe"
$clientHome = "D:\svn-ai-manual-test\client-home"
```

请根据实际安装位置调整变量。

### 2. 创建本地仓库和工作副本

请选择一个尚不存在的测试目录：

```powershell
New-Item -ItemType Directory -Path "D:\svn-ai-manual-test"
svnadmin create "D:\svn-ai-manual-test\repository"
svn checkout "file:///D:/svn-ai-manual-test/repository" "D:\svn-ai-manual-test\working-copy"
```

这里：

- `repository` 是 SVN 仓库数据库，不能直接编辑；
- `working-copy` 是开发人员实际修改代码的工作副本。

### 3. 提交基线版本

```powershell
Set-Location "D:\svn-ai-manual-test\working-copy"
"# SVN AI manual test" | Set-Content -Encoding utf8 "README.md"
svn add "README.md"
svn commit -m "Initial baseline"
```

确认修订号和真实提交人：

```powershell
svnlook youngest "D:\svn-ai-manual-test\repository"
svnlook author -r 1 "D:\svn-ai-manual-test\repository"
```

后续 `--svn-username` 必须与 `svnlook author` 的输出一致。

### 4. 配置客户端

测试密钥可以临时生成；正式环境必须通过组织批准的密钥管理方式统一下发。

```powershell
$key = "07" * 32

& $svnAi --home $clientHome configure `
  --device-id "manual-test-device" `
  --svn-username "Administrator" `
  --fingerprint-key $key `
  --svn $svnExe
```

将 `Administrator` 替换为上一步查到的真实提交人。

检查状态：

```powershell
& $svnAi --home $clientHome status --json
```

首次配置应显示：

```json
{
  "configured": true,
  "device_id": "manual-test-device",
  "svn_username": "Administrator",
  "pending_transactions": 0,
  "attribution_events": 0
}
```

### 5. 安装 Claude Code Hook

先关闭已有 Claude Code 会话，然后执行：

```powershell
& $svnAi --home $clientHome install-hooks
```

默认修改当前用户的：

```text
%USERPROFILE%\.claude\settings.json
```

如果文件原来存在，会创建：

```text
settings.json.svn-ai.bak
```

安装器会保留已有配置和无关 Hook，并注册以下三个事件：

- `PreToolUse`
- `PostToolUse`
- `PostToolUseFailure`

三者的 matcher 都是 `Edit|Write`。中文路径在旧版 Windows PowerShell 中显示乱码时，可使用：

```powershell
Get-Content -Raw -Encoding utf8 "$env:USERPROFILE\.claude\settings.json"
```

不要把包含访问令牌的完整 `settings.json` 发送到聊天、邮件或工单中。

### 6. 让 Claude Code 生成文件

必须从 SVN 工作副本启动一个全新的 Claude Code 会话：

```powershell
Set-Location "D:\svn-ai-manual-test\working-copy"
claude
```

示例提示词：

```text
请使用 Write 工具新建 calculator.py，实现加、减、乘、除四个函数，并提供一个 main 函数演示调用。不要使用 Bash、PowerShell 或重定向命令创建文件，必须使用 Write 工具。
```

Claude 完成后退出，再检查：

```powershell
svn status
& $svnAi --home $clientHome status --json
```

预期：

- `svn status` 显示 `? calculator.py`；
- `pending_transactions` 为 `0`；
- `attribution_events` 至少为 `1`。

### 7. 人工改写并提交

使用记事本或 IDE 手工修改 Claude 生成文件中的两行，不要让 Claude 执行这一步。随后执行：

```powershell
svn add "calculator.py"
svn commit -m "Add calculator created with Claude Code"
```

此时仓库一般产生修订 2。以实际修订号为准。

### 8. 查看归因结果

为避免 PowerShell 续行符粘贴错误，建议先使用单行命令：

```powershell
& $svnAi --home $clientHome stats --repository "D:\svn-ai-manual-test\repository" --revision 2 --svnlook $svnlookExe --json
```

一次真实手工验证的结果如下：

```json
{
  "svn_additions": 42,
  "ai_additions": 38,
  "non_ai_additions": 2,
  "ambiguous_additions": 2
}
```

字段含义：

- `svn_additions`：该修订最终新增的总行数；
- `ai_additions`：与 Claude Code 生成指纹唯一匹配的新增行；
- `non_ai_additions`：没有对应 AI 指纹的新增行；
- `ambiguous_additions`：存在多个相同候选、无法唯一归属的新增行。

重复空白行或其他完全相同的重复行可能进入歧义项。系统不会为了提高 AI 比例而强行把歧义代码归为 AI。

## 常用命令

```powershell
# 查看配置和采集数量
svn-ai status --json

# 查看本地保存的指纹事件
svn-ai events --json

# 安装或补齐 Claude Hook
svn-ai install-hooks

# 统计一个已提交修订
svn-ai stats --repository <仓库物理目录> --revision <修订号> --svnlook <svnlook路径> --json
```

如果使用自定义数据目录，必须把全局参数放在子命令前：

```powershell
svn-ai --home "D:\svn-ai-data" status --json
```

## 隐私边界

最终归因事件包含：

- SVN 仓库 UUID；
- 仓库相对路径；
- SVN 用户名；
- Claude 工具类型；
- 带密钥的代码行指纹和上下文指纹。

最终归因事件不包含：

- Claude 提示词和回答；
- Claude transcript；
- 完整源代码；
- 文件绝对路径；
- 原始 Claude 会话 ID 和工具调用 ID。

源文件修改前快照只短期保存在开发人员本机 SQLite 事务表中，并在成功、失败或取消后删除。

## 开发验证

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Windows + Docker Desktop 真实 SVN 归因测试：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\verify-svn-e2e.ps1
```

Claude Hook → SVN 全链路测试：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\verify-claude-svn-e2e.ps1
```

两条端到端测试的预期结果都是：

```json
{
  "svn_additions": 10,
  "ai_additions": 8,
  "non_ai_additions": 2,
  "ambiguous_additions": 0
}
```

容器当前使用 Subversion 1.14.2。适配器只调用 Subversion 1.7 已提供的 `author`、`date`、`uuid`、`changed --copy-info` 和 `cat` 命令。正式上线前仍需在 VisualSVN Server 2.5.2 主机上使用实际 `svnlook.exe` 做最终冒烟测试。

## 项目结构

- `svn-ai-protocol`：版本化 JSON 事件协议；
- `svn-ai-core`：代码行指纹、差异、来源状态和归因匹配；
- `svn-ai-svn`：SVN 工作副本发现与 `svnlook` 修订读取；
- `svn-ai`：Claude Code Hook、本地 SQLite 和客户端命令；
- `svn-ai-validate`：底层归因诊断工具。

## 设计文档

- [系统设计](docs/superpowers/specs/2026-09-08-svn-ai-attribution-system-design.md)
- [第一阶段实现计划](docs/superpowers/plans/2026-09-08-phase-1-core-protocol.md)
- [Claude Code 客户端实现计划](docs/superpowers/plans/2026-09-14-claude-code-client.md)
