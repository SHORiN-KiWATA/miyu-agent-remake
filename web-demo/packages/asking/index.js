// @ts-check
//! 确认和提问（软件包 `asking`，蓝图 `web.md`「确认和提问」）：抽屉挂进 `composer.takeover`，开着时占着输入框（服务 `composer` 的
//! `takeover`）；了结以后留下的挂进 `chat.tail`，跟着正在看的会话（事件 `session.opened`、`session.created`）；登记
//! `/demo-ask`、`/demo-approve`。核心还推不来（`session.answer` 在 M9）：演示照 `fake.json` 轮着出，作答不发给核心。
//! 一次一个，前一个还没了结又来一个，排在后面。

import { Drawer } from './drawer.js';
import { Reports } from './report.js';
import { openAsk, openApproval, report } from './model.js';

/** @param {any} ctx */
export function apply(ctx) {
  const text = (path, fields) => ctx.text(path, fields);
  const reports = new Reports(text);
  /** 排着的：打开时是哪个会话在问 @type {{d: import('./model.js').Drawer, session: string|null}[]} */
  const queue = [];
  /** 开着的这一个是哪个会话的 */
  let asker = /** @type {string|null} */ (null);
  const drawer = new Drawer(ctx.config, text, (result, d) => {
    // 先把框还回来：后面哪一步出了错，框也不能一直被占着（打不了字、发不了话）
    ctx.composer.takeover(false);
    try {
      const got = report(d, result);
      // 刚答完：正文回到跟着最新的，露出留下的这一条
      if (got && reports.add(asker, got)) requestAnimationFrame(() => ctx.chat.reveal());
      // 取消时在回答：顺带打断这一轮（照 TUI 演示）
      if ('cancelled' in result && ctx.chat.running()) ctx.chat.interrupt();
    } finally {
      next();
    }
  }, () => ctx.chat.home());
  const next = () => {
    const item = queue.shift();
    if (!item) return;
    asker = item.session;
    // 先画好抽屉再占框：框照画好的抽屉量高度（反过来量到的是空的，先缩成一条再跳上去）
    drawer.show(item.d);
    ctx.composer.takeover(true);
    // 占了框、抽屉露出来以后才接得住焦点（藏着的时候给不上）
    drawer.focus();
  };
  const push = (d) => {
    queue.push({ d, session: ctx.chat.current() });
    if (!drawer.open) next();
  };
  ctx.effect(() => () => {
    if (drawer.open) ctx.composer.takeover(false);
  });
  ctx.slots.mount('composer.takeover', { id: 'asking', order: 10, render: () => drawer.el });
  ctx.slots.mount('chat.tail', { id: 'asking', order: 10, render: () => reports.el });
  reports.show(ctx.chat.current());
  ctx.on('session.opened', (id) => reports.show(id));
  ctx.on('session.created', ({ from, to }) => {
    reports.rename(from, to);
    if (asker === from) asker = to;
    for (const item of queue) if (item.session === from) item.session = to;
    reports.show(to);
  });
  // 演示：照 fake.json 轮着出
  let fake = /** @type {Promise<any>|null} */ (null);
  const turns = { asks: 0, approvals: 0 };
  const demo = async (/** @type {'asks'|'approvals'} */ list) => {
    fake ??= fetch(new URL('./fake.json', import.meta.url)).then((r) => r.json());
    const items = (await fake)[list];
    const item = items[turns[list]++ % items.length];
    // 同一个假请求再出一次也当新的：编号接上第几次
    const fresh = { ...item, body: { ...item.body, call_id: `${item.body.call_id}#${turns[list]}` } };
    push(list === 'asks' ? openAsk(fresh) : openApproval(fresh));
  };
  ctx.commands.register({ name: 'demo-ask', summary: ctx.text('command_ask') }, () => demo('asks'));
  ctx.commands.register({ name: 'demo-approve', summary: ctx.text('command_approve') }, () => demo('approvals'));
}
