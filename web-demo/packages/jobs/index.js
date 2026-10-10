// @ts-check
//! 后台任务（软件包 `jobs`，蓝图 `web.md`「后台任务」）：框下面那一行的中间写有几个在跑（挂载位 `composer.footer`），点开
//! 列出这个会话派出去的后台命令、子代理，子代理再派的挂在它下面（挂载位 `composer.float`），在跑的能停（`job.stop`，照派它的会话），
//! 停在半路的子代理单写（`lib/jobs.js`）；后台命令点开看输出（`job.output`）。照正在看的会话的会话状态的任务表（核心 9-8）、读进来了的
//! 子代理的会话状态算：打开时照会话仓库算一次，之后照对话区每画一次的事件 `view.changed`。打断那一轮后面那一句点了发事件 `jobs.open`，
//! 这里打开浮层。停用了按钮和浮层都没有，正文里回报那一行照样有（基础系统画）。

import { tasksFromStatus } from '../../src/lib/jobs.js';
import { JobsPanel } from './panel.js';

/** @param {any} ctx */
export function apply(ctx) {
  const t = (path, fields) => ctx.text(path, fields);
  let session = ctx.chat.current();
  /** 一棵树照会话状态的任务表，子代理的挂着它自己的会话状态（读进来了的，同一个不走两遍） */
  const statusOf = (/** @type {string} */ id) => ctx.sessions.sessions.get(id)?.view.status ?? null;
  const treeOf = (/** @type {string} */ owner, /** @type {any} */ status, /** @type {Set<string>} */ seen) => tasksFromStatus(status?.jobs).map((x) => {
    const own = x.what === 'agent' && x.session && !seen.has(x.session) ? statusOf(x.session) : null;
    return { ...x, owner, kids: own ? treeOf(/** @type {string} */ (x.session), own, new Set([...seen, /** @type {string} */ (x.session)])) : [] };
  });
  /** 上一次算过的：任务表没变的不重画（在收的字每来一段都画一次） */
  let seen = '';
  const panel = new JobsPanel(t, ctx.config, {
    stop: async (owner, job) => {
      try {
        await ctx.core.request('job.stop', { session: owner, job });
      } catch (err) {
        const known = err?.reason ? t(`reasons.${err.reason}`) : null;
        ctx.composer.say(known && known !== `reasons.${err.reason}` ? known : err?.message ?? String(err));
      }
    },
    enter: (id) => ctx.chat.open(id),
    // 后台命令的输出：照派它的会话读最后几行（施工 7-4 补）
    read: (owner, job) => ctx.core.request('job.output', { session: owner, job, tail: ctx.config.preview_lines }),
  });
  ctx.effect(() => () => panel.destroy());
  /** 会话状态的任务表变了才重画 @param {string|null} id @param {any} status */
  const redo = (id, status) => {
    if (id !== session) panel.close();
    session = id;
    const sig = `${id}|${JSON.stringify(status?.jobs ?? [])}|${[...ctx.sessions.sessions.values()].map((s) => JSON.stringify(s.view.status?.jobs ?? [])).join()}`;
    if (sig === seen) return;
    seen = sig;
    panel.update(id ? treeOf(id, status, new Set([id])) : []);
  };
  redo(session, session ? statusOf(session) : null);
  ctx.on('view.changed', (v) => redo(v.session, v.status));
  ctx.on('jobs.open', () => panel.open());
  ctx.slots.mount('composer.footer', { id: 'jobs', order: 10, render: () => panel.button });
  ctx.slots.mount('composer.float', { id: 'jobs', order: 10, render: () => panel.el });
}
