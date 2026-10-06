// @ts-check
//! 「软件包」页（蓝图 `web.md`「设置页」第 13 条，原来 `/pkg` 的浮层）：每个可选的包一行：名字、一句给人看的说明（清单的 `summary`；
//! `description` 是写给开发的）、开关；好好的不写状态，等着的、故障的才写一行为什么。基础系统的包（停不了）不列，编号、提供的服务
//! 不显示。有设置项的点这一行展开它的设置。

import { h, icon, replace } from '../../src/lib/dom.js';
import { toggle } from './rows.js';
import { packageSettings } from './look.js';
import { editableKeys } from './model.js';

/** 展开着的包：换页回来还开着（这个弹窗里记着） */
const opened = new Set();

export function drawPackages(dialog) {
  const ctx = dialog.ctx;
  const el = h('div.set-rows');
  const draw = async () => {
    const all = (await ctx.packages.list()).filter((p) => p.manifest?.kind !== 'base' && p.id !== 'settings');
    replace(el, all.map((p) => {
      const m = p.manifest ?? {};
      const why = p.state === 'pending' && p.missing.length ? ctx.text('pkg_missing', { names: p.missing.join('、') }) : p.state === 'failed' ? p.reason : '';
      const on = p.state !== 'disabled';
      // 能改的设置项一项都没有的（全是结构复杂的）不出展开的箭头
      const hasSettings = editableKeys(m.settings).length > 0;
      const body = h('div.set-pkg-settings', { hidden: !opened.has(p.id) }, opened.has(p.id) ? packageSettings(dialog, p.id) : null);
      const head = h(`div.set-row.set-pkg${hasSettings ? '.is-expandable' : ''}${opened.has(p.id) ? '.is-open' : ''}`, {
        onclick: (e) => {
          if (!hasSettings || e.target.closest('.set-switch')) return;
          const open = !opened.has(p.id);
          if (open) opened.add(p.id);
          else opened.delete(p.id);
          head.classList.toggle('is-open', open);
          body.hidden = !open;
          if (open && !body.childElementCount) body.append(packageSettings(dialog, p.id));
        },
      },
      h('span.set-pkg-arrow', hasSettings ? icon('chevron-right') : null),
      h('div.set-text',
        h('div.set-name', h('span', ctx.local(m.name ?? p.id))),
        m.summary ? h('p.set-desc', ctx.local(m.summary)) : null,
        why ? h('p.set-problem.is-error', why) : null),
      h('div.set-control', toggle(on, async () => {
        await ctx.packages.set(p.id, { disabled: on });
        await draw();
      })));
      return h('div.set-pkg-block', head, body);
    }));
  };
  draw().catch((err) => replace(el, h('p.set-empty.is-bad', err.message)));
  return h('section.set-group', el);
}
