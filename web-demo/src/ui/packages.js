// @ts-check
//! 软件包的浮层（`/pkg`，蓝图 `web.md`「斜杠命令」、`web/architecture.md`「设置页」）：列出每个包的名字、编号、状态，可选的
//! 包点开关当场停用、启用（内核的服务 `packages`，记在这个浏览器里）。设置页做好以后挪进去。

import { h } from './dom.js';
import { leave } from '../lib/motion.js';
import { t, res } from '../util/res.js';
import { local } from '../lib/text.js';

/**
 * 打开浮层。`Esc`、点外面、点「关闭」关上。
 * @param {{list: () => Promise<any[]>, set: (id: string, patch: any) => Promise<void>}} packages 内核的服务
 * @param {(text: string) => void} say 出错时提示一句
 */
export async function openPackages(packages, say) {
  const list = h('div.pkg-list');
  // 收起：遮罩淡出、卡片缩回去，走完再拿掉（蓝图「动效」）
  const close = () => {
    document.removeEventListener('keydown', onKey, true);
    leave(root, () => root.remove());
  };
  const onKey = (/** @type {KeyboardEvent} */ e) => {
    if (e.key !== 'Escape') return;
    e.stopPropagation();
    close();
  };
  const root = h('div.pkg-layer',
    h('button.pkg-scrim', { type: 'button', 'aria-label': t('pkg.close'), onclick: close }),
    h('section.pkg-panel', { role: 'dialog', 'aria-label': t('pkg.title') },
      h('header.pkg-head', h('strong', t('pkg.title')), h('span', t('pkg.note'))),
      list));
  const draw = async () => {
    const rows = await packages.list();
    list.replaceChildren(...rows.map((p) => row(p, async (disabled) => {
      try {
        await packages.set(p.id, { disabled });
      } catch (err) {
        say(err.message);
      }
      await draw();
    })));
  };
  document.body.append(root);
  document.addEventListener('keydown', onKey, true);
  await draw();
}

/** 一行：名字、编号、状态（等着的写缺什么，故障的写为什么），可选的有开关。 */
function row(p, toggle) {
  const name = local(p.manifest?.name ?? p.id, res.language);
  const why = p.state === 'pending' && p.missing.length ? t('pkg.missing', { names: p.missing.join(t('list_sep')) }) : p.state === 'failed' ? p.reason : '';
  const base = p.manifest?.kind === 'base';
  const on = p.state !== 'disabled';
  return h(`div.pkg-row.is-${p.state}`,
    h('div.pkg-name', h('span', name), h('code', p.id)),
    h('div.pkg-state', h('span.pkg-chip', t(`pkg.states.${p.state}`)), why ? h('span.pkg-why', { title: why }, why) : null),
    base
      ? h('span.pkg-base', t('pkg.base'))
      : h(`button.pkg-switch${on ? '.is-on' : ''}`, { type: 'button', role: 'switch', 'aria-checked': String(on), title: t(on ? 'pkg.disable' : 'pkg.enable'), onclick: () => toggle(on) }, h('i')));
}
