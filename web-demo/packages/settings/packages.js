// @ts-check
//! 「软件包」页（蓝图 `web.md`「设置页」第 13 条，原来 `/pkg` 的浮层）：每个可选的包一行：名字、一句给人看的说明（清单的 `summary`；
//! `description` 是写给开发的）、开关；好好的不写状态，等着的、故障的才写一行为什么。基础系统的包（停不了）不列，编号、提供的服务
//! 不显示。有设置项的点这一行滑进它的设置页（`subpage.js`，左上角箭头返回）。

import { h, icon, replace } from '../../src/lib/dom.js';
import { toggle } from './rows.js';
import { packageSettings } from './look.js';
import { editableKeys } from './model.js';

export function drawPackages(dialog) {
  const ctx = dialog.ctx;
  const el = h('div.set-rows');
  const draw = async () => {
    const all = (await ctx.packages.list()).filter((p) => p.manifest?.kind !== 'base' && p.id !== 'settings');
    replace(el, all.map((p) => {
      const m = p.manifest ?? {};
      const why = problemOf(ctx, p);
      const on = p.state !== 'disabled';
      // 有能改的设置项的点这一行滑进它的设置页（2026-10-10 项目主人），右边多一个箭头；一项都没有的（全是结构复杂的）不能点
      const hasSettings = editableKeys(m.settings).length > 0;
      return h(`div.set-row.set-pkg${hasSettings ? '.is-expandable' : ''}`, {
        onclick: (e) => {
          if (!hasSettings || e.target.closest('.set-switch')) return;
          dialog.openSub(ctx.local(m.name ?? p.id), () => webPackagePage(dialog, p.id));
        },
      },
      h('div.set-text',
        h('div.set-name', h('span', ctx.local(m.name ?? p.id))),
        m.summary ? h('p.set-desc', ctx.local(m.summary)) : null,
        why ? h('p.set-problem.is-error', why) : null),
      h('div.set-control', toggle(on, async () => {
        await ctx.packages.set(p.id, { disabled: on });
        await draw();
      })),
      // 没有设置页的也留着箭头那一格：开关上下对齐
      h('span.set-pkg-arrow', hasSettings ? icon('chevron-right') : null));
    }));
  };
  draw().catch((err) => replace(el, h('p.set-empty.is-bad', err.message)));
  return h('section.set-group', el);
}

/** 等着的、故障的包为什么（好好的是空的）。 @param {any} ctx @param {any} p */
const problemOf = (ctx, p) => (p.state === 'pending' && p.missing.length ? ctx.text('pkg_missing', { names: p.missing.join('、') }) : p.state === 'failed' ? p.reason : '');

/**
 * 网页一个软件包的设置页（点进去的那一层）：顶上一句说明，「启用」一行开关，再是它的设置项（`look.js` 的 `packageSettings`）。
 * @param {any} dialog @param {string} id
 */
function webPackagePage(dialog, id) {
  const ctx = dialog.ctx;
  const box = h('div');
  ctx.packages.list().then((all) => {
    const p = all.find((x) => x.id === id);
    if (!p) return;
    const m = p.manifest ?? {};
    const why = problemOf(ctx, p);
    replace(box,
      m.summary ? h('p.set-sub-desc', ctx.local(m.summary)) : null,
      h('section.set-group', h('div.set-rows', h('div.set-row',
        h('div.set-text', h('div.set-name', h('span', ctx.text('enabled'))), why ? h('p.set-problem.is-error', why) : null),
        h('div.set-control', toggle(p.state !== 'disabled', async () => {
          await ctx.packages.set(id, { disabled: p.state !== 'disabled' });
          dialog.drawBody();
        }))))),
      packageSettings(dialog, id));
  }).catch((err) => replace(box, h('p.set-empty.is-bad', err.message)));
  return [box];
}
