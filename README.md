# Node Tide（拾海）

Node Tide 是一款本地优先的 Windows 知识工作台，用来保存、检索、阅读和评估文章、GitHub 仓库与 Prompt。中文名“拾海”表达的是：从信息之海中拾取真正值得留下的内容。

> 当前处于邀请制 Windows Alpha。不要把它用于唯一副本、公司机密、客户数据或任何无法安全恢复的资料。

## 安装 Alpha

1. 从受信任的发布渠道取得 `.msi` 或 `-setup.exe` 安装包，以及同一候选版本的 SHA-256 清单。
2. 在 PowerShell 中校验下载文件：

   ```powershell
   Get-FileHash .\Node.Tide_0.1.0_x64-setup.exe -Algorithm SHA256
   ```

3. 只在结果与发布清单完全一致时安装。当前 Alpha 可能未签名；Windows 出现来源提示时，先重新核对文件名、版本和哈希，不要盲目放行。
4. 第一次启动后先完成一个最小闭环：保存一个公开 URL，用标题或正文关键词找到它，再打开结果运行一次 Evaluation。

AI 不是前置条件。没有配置模型时，保存、阅读和本地搜索仍然可用；需要分析时再从 **Settings → Models** 配置 BYO API 或本地 Ollama。

## 数据、备份与隐私

- 默认数据保存在本机，不要求云账号，也不默认上传遥测。
- API key 不应出现在截图、反馈文本、日志或支持包中。
- 升级、恢复或大量导入前先备份；操作步骤见 [备份与恢复](docs/backup_and_restore.md)。
- 只有在应用内明确确认后才生成支持包，并应由你检查后主动选择是否分享。
- 安全与隐私边界见 [安全策略](docs/security_and_privacy_policies.md)。

## 已知 Alpha 限制

- 目前只支持 Windows 10/11。
- 安装包可能未签名，浏览器扩展及外部网络链路仍可能存在环境差异。
- 没有云同步、移动端、团队空间和托管模型网关。
- Alpha 不承诺数据格式永久兼容；请始终保留可验证的备份。

## 反馈

邀请用户请按 [Alpha 反馈手册](docs/alpha_feedback_playbook.md) 提交脱敏记录。不要提交 API key、token、cookie、session、password、完整正文、Prompt 原文、source snapshot、embedding、完整 URL/query 或本机绝对路径。

遇到数据丢失、凭据泄漏、无法启动且无法恢复、误删数据或安装包来源不可验证时，按 P0 处理并立即停止继续邀请。

## 本地开发

需要 Node.js `20.19+`、`22.13+` 或 `24+`，以及 Rust `1.85+`。

```powershell
npm ci
npm run lint
npm run typecheck
npm test
npm run build
npm run test:e2e:install
npm run test:e2e
```

完整 Alpha 候选验证使用：

```powershell
npm run readiness:alpha
```

架构与开发入口见 [项目文档索引](docs/README.md)。
