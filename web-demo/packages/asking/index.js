// @ts-check
//! 确认和提问（软件包 `asking`，蓝图 `web.md`「确认和提问」）：抽屉挂进 `composer.takeover`，开着时占着输入框（服务 `composer` 的
//! `takeover`）。数据照正在看的会话的会话状态（核心 9-8 的 `waiting`；D-1 `session.answer`、D-2 `ask_user`）：在等的确认、提问
//! 一个一个开，答了发 `session.answer`，取消是打断这一轮；别处先答了，开着的当场收掉。了结以后留下的由正文那一层照条目排、交给挂载位
//! `chat.item`（键 `asking`）由这里画，夹在她这一轮里，刷新、别的设备照样有。

import { Drawer } from './drawer.js';
import { Reports } from './report.js';
import { pendingFromStatus, reportOf } from './model.js';

/** @param {any} ctx */
export function apply(ctx) {
  const text = (path, fields) => ctx.text(path, fields);
  const reports = new Reports(text);
  // 了结以后留下的：正文的条目里一条（`slot: 'asking'`，带着了结的和问的那两条），画成卡片、一行；允许了的画空的
  ctx.slots.mount('chat.item', {
    id: 'asking',
    key: 'asking',
    render: (/** @type {any} */ it, /** @type {{fresh: boolean}} */ how) => {
      const got = reportOf(it.event, it.asked);
      return got ? reports.node(got, how.fresh) : document.createElement('div');
    },
  });
  /** 答了、取消了，等核心回应的调用：这时不再开。调用编号只在一个会话里唯一（换个会话从头编），照「会话 + 调用」认 */
  const answering = new Set();
  const keyOf = (/** @type {string|null} */ session, /** @type {string} */ call) => `${session ?? ''} ${call}`;
  /** 开着的这一个 @type {{d: import('./model.js').Drawer, session: string|null}|null} */
  let showing = null;
  /** 最近一次对话区画的（事件 `view.changed`）：会话、会话状态、条目 @type {{session: string|null, status: any, entries: any[]}|null} */
  let view = null;

  const drawer = new Drawer(ctx.config, text, (result, d) => {
    // 先把框还回来：后面哪一步出了错，框也不能一直被占着（打不了字、发不了话）
    ctx.composer.takeover(false);
    const item = showing;
    showing = null;
    try {
      if (item) send(item.session, d, result);
    } finally {
      next();
    }
  }, () => ctx.chat.home());

  /** 答了发 `session.answer`，取消打断这一轮（核心把在等的这一问记成取消）；拒了写一句（别处先答了的不写）。 */
  const send = async (session, d, result) => {
    answering.add(keyOf(session, d.id));
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
      answering.delete(keyOf(session, d.id));
      // 核心先推后回应：这时会话状态已经不等这一问了；拒了的照会话状态还在等，再开
      if (view) sync(view);
    }
  };

  /** 还没了结的：会话状态的 `waiting`（核心 9-8）。 @param {any} v */
  const pending = (v) => pendingFromStatus(v.status, v.entries);

  /** 开着的没有了，开下一个：正在看的会话里还没了结的（不算正在等回应的）。 */
  const next = () => {
    if (drawer.open) return;
    const real = view ? pending(view).find((/** @type {any} */ d) => !answering.has(keyOf(view?.session ?? null, d.id))) : null;
    if (!real) return;
    showing = { d: real, session: view?.session ?? null };
    // 先画好抽屉再占框：框照画好的抽屉量高度（反过来量到的是空的，先缩成一条再跳上去）
    drawer.show(real);
    ctx.composer.takeover(true);
    // 占了框、抽屉露出来以后才接得住焦点（藏着的时候给不上）
    drawer.focus();
  };

  /** 会话状态变了：开着的那一个别处了结了、换了会话，收掉（不留提示）；再看要不要开下一个。 */
  const sync = (/** @type {{session: string|null, status: any, entries: any[]}} */ v) => {
    view = v;
    if (showing) {
      const still = showing.session === v.session && pending(v).some((/** @type {any} */ d) => d.id === showing?.d.id);
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
  ctx.on('view.changed', (/** @type {any} */ v) => sync({ session: v.session ?? null, status: v.status ?? null, entries: v.entries ?? [] }));
}
