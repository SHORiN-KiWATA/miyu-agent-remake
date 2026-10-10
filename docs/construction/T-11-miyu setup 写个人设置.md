## 施工单 T-11：`miyu setup` 写个人设置

状态：2026-10-10 合了（e529e091，CI run 1591；主会话定；网页的会话在第一次引导里碰上、转来）。

### 起因

项目主人在网页的第一次引导里选了 DeepSeek，会话照旧用原来的模型：引导照 `cli/setup.md` 第 10 条写系统配置，个人设置里已经有 `models.chat`，个人设置盖住了系统配置。网页已经改成写个人设置，和设置页一样。`miyu setup` 也写系统配置，个人设置里有 `models.chat` 的人跑完，屏幕上说「写好了」，实际不生效。

### 这一步做什么

1. `miyu setup` 的 `config.set` 写个人设置（`crates/miyu-cli/src/setup/flow.rs`）：`providers.<编号>.*`、`models.chat`、三个预设的池都写在那里。系统配置留给以后多用户时管理员定全机的默认值。
2. 图纸 `cli/setup.md` 第 10 条照改，写明为什么。
3. 测试改成查个人设置；「原来的一个字没动」改成系统配置原样不动。

### 验收

- 新加 `a_chat_model_in_personal_settings_is_replaced`：个人设置里原来有 `models.chat` 的，跑完换成新选的、系统配置不碰。把写的层改回系统配置时它红（复现网页碰上的那一回），改过来绿。
- `cargo xtask check` 八项；三台 CI。
