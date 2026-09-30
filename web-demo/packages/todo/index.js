// @ts-check
//! 待办（软件包 `todo`，蓝图 `web.md`「待办」）：挂进输入框上面的挂载位 `composer.above`（排在运行状态行前面）；跟着正在看的
//! 会话（事件 `session.opened`、`session.created`）；登记 `/demo-todo`。停用了这一块和命令都没了。

import { TodoDock } from './dock.js';

/** @param {any} ctx */
export function apply(ctx) {
  const dock = new TodoDock(ctx.config, (path, fields) => ctx.text(path, fields));
  ctx.effect(() => () => dock.destroy());
  dock.show(ctx.chat.current());
  // 开了一个新会话：它不带着别人的待办；换了会话：它的露出来
  ctx.on('session.opened', (id) => {
    if (id === null) dock.drop(null);
    dock.show(id);
  });
  // 新会话第一句话发出去、会话开了：演示跟过去（还是同一段对话）
  ctx.on('session.created', ({ from, to }) => {
    dock.rename(from, to);
    dock.show(to);
  });
  ctx.slots.mount('composer.above', { id: 'todo', order: 10, render: () => dock.el });
  ctx.commands.register({ name: 'demo-todo', summary: ctx.text('command') }, () => dock.start(ctx.chat.current()));
}
