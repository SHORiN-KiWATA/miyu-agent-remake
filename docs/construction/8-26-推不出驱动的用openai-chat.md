## 施工单 8-26：推不出驱动的用 `openai-chat`

状态：待验收（2026-10-07 写、施工完；项目主人同一天在配置页验收时撞上三次、定的：没写 `driver`、又推不出来的供应商，用 `openai-chat`，配置里照旧不写，`model.list` 标出是默认的；经终端界面的会话转来；技术细节主会话定）。

### 目的

局域网、自建的中转（例如 `http://<局域网>:3425/v1`）只写了地址，目录对不上（编号、相近的编号、地址都对不上），以前这一家用不了：`needs driver and base_url: it matches nothing in the catalog`，一个模型都没有。这类中转多半说 OpenAI 的接口，改成照 `openai-chat` 接上。

### 蓝图改哪几节

- `models.md`「怎么走」第一条第 2 条：有地址、推不出驱动的用 `openai-chat`；连地址都没有的照旧用不了，原话改成只说缺地址。「对外的样子」`driver` 那一行、「协议」`model.list` 每一家多一格 `driver_from`、「出错」那一行、两份样本跟着改。

### 定了的（技术细节，主会话定）

1. 默认的驱动是 `openai-chat`（`miyu_models::provider::DEFAULT_DRIVER`）。只在手写、档案、目录三处都推不出时用。
2. 配置里照旧不写：以后目录对上了、档案写了，照它们的（`Provider::driver_from` 每次照这一轮的配置和资料重算）。
3. `model.list` 每一家多一格 `driver_from`：`config`、`profile`、`catalog`、`default`。用不了的那一家没有这一格。
4. 连地址都推不出的，原话 `provider "<编号>" needs base_url: it matches nothing in the catalog`。
5. 「驱动从哪来」不改模型那一层：模型照目录写了自己的包名的，照旧压过供应商的（`Provider::for_model`，8-14）。

### 不做什么

- 头上怎么显示「默认」：终端界面、网页各自照 `driver_from` 画。
- 自动探测中转说哪种接口（先试 `openai-chat` 再试别的）：要发请求，不做。

### 验收

1. 测试：只写了地址的那一家能用、走 `openai-chat`、来源是默认，手写了驱动的照手写的（`provider/tests.rs` 新的一条，改之前当场报错）；连地址都没有的五处测试换成新原话；`model.list` 的 `driver_from`（档案推出来的是 `profile`）。
2. 手写变异、`cargo xtask check` 八项、三台机器的 CI 和长跑。
3. 合进 main 以后告诉两个头：`driver_from`，原话改了。

### 风险

- 写错了地址的那一家，以前当场说用不了，现在照 `openai-chat` 发出去才出错：`provider.test` 一试就知道，配置页存完照旧会刷列表（`model.list` 带 `refresh`），拉不到列表照样看得出来。
