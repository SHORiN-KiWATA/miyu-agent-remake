// @ts-check
//! 确认和提问（软件包 `asking`，蓝图 `web.md`「确认和提问」）：抽屉挂进 `composer.takeover`，开着时占着输入框（服务 `composer` 的
//! `takeover`）。数据照正在看的会话的事件（核心 D-1 `session.answer`、D-2 `ask_user`）：还没了结的 `question.asked`、
//! `tool.approval_requested` 一个一个开，答了发 `session.answer`，取消是打断这一轮；别处先答了，开着的当场收掉。了结以后留下的由正文
//! 那一层照日志排、交给挂载位 `chat.item`（键 `asking`）由这里画，夹在她这一轮里，刷新、别的设备照样有。
//! `/demo-ask`、`/demo-approve` 照 `fake.json` 出一个看样子：答了只在这一页里，不发给核心。

import { Drawer } from './drawer.js';
import { Reports } from './report.js';
import { openAsk, openApproval, report, pendingAsks, reportOf } from './model.js';

/** @param {any} ctx */
export function apply(ctx) {
  const text = (path, fields) => ctx.text(path, fields);
  const reports = new Reports(text, { anchor: (node) => ctx.chat.anchor(node), place: (where, node) => ctx.chat.place(where, node) });
  // 了结以后留下的：正文的条目里一条（`slot: 'asking'`，带着了结的和问的那两条），画成卡片、一行；允许了的画空的
  ctx.slots.mount('chat.item', {
    id: 'asking',
    key: 'asking',
    render: (/** @type {any} */ it, /** @type {{fresh: boolean}} */ how) => {
      const got = reportOf(it.event, it.asked);
      return got ? reports.node(got, how.fresh) : document.createElement('div');
    },
  });
  /** 演示出的，排在真的后面 @type {{d: import('./model.js').Drawer, session: string|null, demo: true}[]} */
  const demos = [];
  /** 答了、取消了，等核心回应的调用：这时不再开 */
  const answering = new Set();
  /** 开着的这一个 @type {{d: import('./model.js').Drawer, session: string|null, demo: boolean}|null} */
  let showing = null;
  /** 最近一次对话区画的（事件 `view.changed`）：会话和日志 @type {{session: string|null, events: any[]}|null} */
  let view = null;

  const drawer = new Drawer(ctx.config, text, (result, d) => {
    // 先把框还回来：后面哪一步出了错，框也不能一直被占着（打不了字、发不了话）
    ctx.composer.takeover(false);
    const item = showing;
    showing = null;
    try {
      if (item?.demo) local(item, d, result);
      else if (item) send(item.session, d, result);
    } finally {
      next();
    }
  }, () => ctx.chat.home());

  /** 演示的：照原来的样子留在这一页里；取消时在回答顺带打断。 */
  const local = (item, d, result) => {
    const got = report(d, result);
    if (got && reports.addLocal(item.session, got)) requestAnimationFrame(() => ctx.chat.reveal());
    if ('cancelled' in result && ctx.chat.running()) ctx.chat.interrupt();
  };

  /** 真的：答了发 `session.answer`，取消打断这一轮（核心把在等的这一问记成取消）；拒了写一句（别处先答了的不写）。 */
  const send = async (session, d, result) => {
    answering.add(d.id);
    try {
      if ('cancelled' in result) await ctx.chat.interrupt();
      else {
        const answer = result.kind === 'approve'
          ? { decision: result.decision, ...(result.reason ? { reason: result.reason } : {}) }
          : { answers: result.answers };
        await ctx.core.request('session.answer', { session, call: d.id, ...answer });
      }
    } catch (err) {
      if (err?.reason !== 'not_asking') ctx.composer.say(err?.message ?? String(err));
    } finally {
      answering.delete(d.id);
      // 核心先推事件后回应：这时日志里已经有了结的那一条；拒了的照日志还没了结，再开
      if (view) sync(view);
    }
  };

  /** 告诉运行状态行在不在等你（状态事件 `asking.waiting`：开着的是真的才算，演示的不算）。 */
  const announce = () => ctx.publish('asking.waiting', showing && !showing.demo ? { session: showing.session, kind: showing.d.kind } : null);

  /** 开着的没有了，开下一个：正在看的会话里还没了结的（不算正在等回应的），再是演示的。 */
  const next = () => {
    if (drawer.open) return announce();
    const real = view ? pendingAsks(view.events).find((d) => !answering.has(d.id)) : null;
    const item = real ? { d: real, session: view?.session ?? null, demo: false } : demos.shift();
    if (!item) return announce();
    showing = item;
    // 先画好抽屉再占框：框照画好的抽屉量高度（反过来量到的是空的，先缩成一条再跳上去）
    drawer.show(item.d);
    ctx.composer.takeover(true);
    // 占了框、抽屉露出来以后才接得住焦点（藏着的时候给不上）
    drawer.focus();
    announce();
  };

  /** 日志变了：开着的真的那一个别处了结了、换了会话，收掉（不留提示）；再看要不要开下一个；留下的照日志钉好。 */
  const sync = (/** @type {{session: string|null, events: any[]}} */ v) => {
    view = v;
    if (showing && !showing.demo) {
      const still = showing.session === v.session && pendingAsks(v.events).some((d) => d.id === showing?.d.id);
      if (!still) {
        showing = null;
        ctx.composer.takeover(false);
        drawer.close();
      }
    }
    next();
  };

  ctx.effect(() => () => {
    if (drawer.open) ctx.composer.takeover(false);
  });
  ctx.slots.mount('composer.takeover', { id: 'asking', order: 10, render: () => drawer.el });
  ctx.on('view.changed', (/** @type {any} */ v) => sync({ session: v.session ?? null, events: v.events ?? [] }));
  reports.show(ctx.chat.current());
  ctx.on('session.opened', (id) => reports.show(id));
  ctx.on('session.created', ({ from, to }) => {
    reports.rename(from, to);
    for (const item of demos) if (item.session === from) item.session = to;
    if (showing?.session === from) showing.session = to;
    reports.show(to);
  });

  // 演示：照 fake.json 轮着出，排在真的后面
  let fake = /** @type {Promise<any>|null} */ (null);
  const turns = { asks: 0, approvals: 0 };
  const demo = async (/** @type {'asks'|'approvals'} */ list) => {
    fake ??= fetch(new URL('./fake.json', import.meta.url)).then((r) => r.json());
    const items = (await fake)[list];
    const item = items[turns[list]++ % items.length];
    // 同一个假请求再出一次也当新的：编号接上第几次
    const fresh = { ...item, body: { ...item.body, call_id: `${item.body.call_id}#${turns[list]}` } };
    demos.push({ d: list === 'asks' ? openAsk(fresh) : openApproval(fresh), session: ctx.chat.current(), demo: true });
    next();
  };
  ctx.commands.register({ name: 'demo-ask', summary: ctx.text('command_ask') }, () => demo('asks'));
  ctx.commands.register({ name: 'demo-approve', summary: ctx.text('command_approve') }, () => demo('approvals'));
}
