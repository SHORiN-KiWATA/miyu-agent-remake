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
  // 斜杠命令：别的包登记自己的（`/demo-todo` 这类）
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
  });
  ctx.effect(() => () => ctx.page.root.replaceChildren());
}
