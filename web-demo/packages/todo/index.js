// @ts-check
//! 待办（软件包 `todo`，蓝图 `web.md`「待办」）：挂进输入框上面的挂载位 `composer.above`（排在运行状态行前面）；跟着正在看的
//! 会话（事件 `session.opened`、`session.created`）；清单照对话区每画一次带来的（`view.changed` 的 `todos`，核心 D-3）。

import { TodoDock } from './dock.js';

/** @param {any} ctx */
export function apply(ctx) {
  const dock = new TodoDock(ctx.config, (path, fields) => ctx.text(path, fields));
  ctx.effect(() => () => dock.destroy());
  dock.show(ctx.chat.current());
  // 换了会话：它的露出来；新会话第一句话发出去、会话开了：跟过去
  ctx.on('session.opened', (id) => dock.show(id));
  ctx.on('session.created', ({ to }) => dock.show(to));
  // 待办（核心 D-3）：对话区每画一次带着正在看的会话的清单
  ctx.on('view.changed', (/** @type {any} */ v) => dock.setReal(v.session ?? null, v.todos ?? [], v.todosDone ?? null));
  ctx.slots.mount('composer.above', { id: 'todo', order: 10, render: () => dock.el });
}
