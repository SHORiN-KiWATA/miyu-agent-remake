// @ts-check
//! 整页（蓝图 `web/architecture.md`「怎么搬」第 2 步）：拆成软件包之前的整个页面，先当一个基础系统的包跑起来；以后一件件
//! 拆出去，拆完这个包就没了。它是搬的过程里唯一还直接 `import` 旧代码（`src/ui/`、`src/core/`）的包。

import { App } from '../../src/ui/app.js';
import { useConnection as linkCardsUse } from '../../src/ui/linkcards.js';
import { codeBlock, copy } from '../../src/markdown/build.js';

/** @param {any} ctx */
export function apply(ctx) {
  // 链接卡片经这条连接问桥
  linkCardsUse(ctx.core);
  // 图片点开找灯箱：用的时候再找（`lightbox~`），灯箱装上、停了整页不重来
  const app = new App(ctx.sessions, ctx.core, ctx.host, ctx, () => ctx.lightbox);
  app.mount(ctx.page.root);
  // 对话区：别的包经它拿滚的那一层、正文那一列、你说的话
  // Markdown 的代码块、复制：mermaid 这类包经它用
  ctx.provide('markdown', { codeBlock, copy });
  // 斜杠命令：别的包登记自己的（`/settings`、`/pkg` 这类）
  ctx.provide('commands', app.commands);
  ctx.provide('chat', {
    /** 正在看的会话（还没开的新会话是 `null`） */
    current: () => app.current,
    scroller: app.chat.el,
    list: app.chat.list,
    onPrompts: (fn) => app.chat.onPrompts(fn),
    /** 看另一个会话（子代理的会话也行，第一次看时才读） */
    open: (id) => app.open(id),
    /** 正在看的会话在不在回答；打断它（照两下 `Esc`） */
    running: () => !!app.composer.running,
    /** 家目录（握手回应的 `host.home`）：路径写成 `~` 用 */
    home: () => app.home,
    interrupt: () => app.interrupt(),
    /** 正文末尾（挂载位 `chat.tail`）来了新的：回到跟着最新的、露出它 */
    reveal: () => app.chat.reveal(),
    /** 钉一个节点在这时正文里最后一块的后面，交回钉在哪；照交回的位置再钉（换了会话回来） */
    anchor: (node) => app.chat.anchor(node),
    place: (where, node) => app.chat.place(where, node),
    /** 正在看的会话在哪个目录干活；账号的工作区（握手回应的 `host.workspace`，新会话默认在这里） */
    workdir: () => app.workdir(),
    defaultWorkdir: () => app.cwd,
    /** 还没开的新会话选的人格、预设、工作区：开会话时带上（事件 `draft.changed`） */
    draft: () => ({ ...app.draft }),
    setDraft: (patch) => app.setDraft(patch),
    /** 换这个会话在哪干活（核心 9-7 的 `session.set_workspace`）：交回实际的目录，拒了的抛出来；太宽退回工作区的来事件 `workdir.adjusted` */
    setWorkdir: (session, cwd) => app.setWorkdir(session, cwd),
    /** 会话用的人格（核心 P-1 下：`session.created` 的 `persona`，没读进来的照会话表那一项的；以前的日志没有，是 `null`） */
    persona: (id) => app.store.sessions.get(id)?.events.find((e) => e.kind === 'session.created')?.body.persona ?? app.store.index.get(id)?.persona ?? null,
    /** 会话用的人格画成什么样：交一个函数（会话 → `{name, avatar}`；无人格 `null`；还不知道 `undefined`），软件包 `setup` 给 */
    look: (fn) => app.setLooker(fn),
    /** 人格列表、新会话选的人格变了：照 `look` 重新取一次 */
    refreshLook: () => app.syncLook(),
    /** 会话用的预设（核心 P-2：`session.created` 的 `preset`，没读进来的照会话表；以前的会话没有，是 `null`） */
    preset: (id) => app.store.sessions.get(id)?.events.find((e) => e.kind === 'session.created')?.body.preset ?? app.store.index.get(id)?.preset ?? null,
  });
  // 输入框：提示、跟着发的东西变了、写字的那个框（附件这类包经它粘贴、提示）
  ctx.provide('composer', {
    say: (text, good) => app.composer.say(text, good),
    changed: () => app.composer.syncButton(),
    input: app.composer.input,
    /** 整个框：附件照它收拖进来的文件 */
    box: app.composer.box,
    focus: () => app.composer.focus(),
    /** 挂载位 `composer.takeover` 占不占着框（确认和提问的抽屉） */
    takeover: (open) => app.composer.takeover(open),
    /** 锁住输入框（`text` 是占位字，`null` 解锁）：默认的人格没了、还没选的时候（软件包 `setup`） */
    lock: (text) => app.composer.lock(text),
  });
  ctx.effect(() => () => ctx.page.root.replaceChildren());
}
