## 施工单 T-12：常用供应商照 opencode

状态：2026-10-10 合了（abda7280，CI run 1608；变基到 R-11 以后本地八项重跑过）。项目主人同一天定，终端界面的会话转来。

### 这一步做什么

1. `resources/models/featured.toml` 照 opencode 的常用供应商改，国内外分开的各列一行，不再写 `catalog_zh` 按界面语言自动换（读它的代码留着）；Kimi 那一家叫 Moonshot；加 OpenCode Go。一共 14 家：OpenCode Zen、OpenCode Go、DeepSeek、Anthropic、OpenAI、OpenRouter、Moonshot 国际版和国内版、智谱 GLM 国内版、Z.AI 国际版、通义千问国际版和国内版、MiniMax 国际版和国内版。
2. 不列的：Google（目录里走 Gemini 自己的接口，核心没有这种驱动，列出来第一次接入时只会写「用不了」；等有 key 实测、档案里配上它的兼容接口再加）、GitHub Copilot（要 OAuth 登录）、Vercel（网关没实测）。
3. 图纸 `models.md`、`cli/setup.md` 的样子照改。

### 验收

- `miyu-core` 的 `every_featured_provider_is_usable_with_the_bundled_data`：14 家拿出厂的目录、档案都接得上（改成 14）。
- `miyu setup` 的测试照新的编号、新的样子改（DeepSeek 从 1 变 3，自定义从 3 变 4）。
- `cargo xtask check` 八项；三台 CI。
