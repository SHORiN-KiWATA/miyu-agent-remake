// @ts-check
//! 选模型的小窗（蓝图 `web.md`「设置页」第 14 条，模型池「＋」）：浮在按钮旁边，顶上搜索框，下面一行一个模型（勾选框、显示名、
//! 模型名、哪一家），能勾好几个；底下「取消」「加入 N 个」，照勾的先后交出去。`Esc`、点外面关（类名 `set-menu`：弹窗的 `Esc` 先关它）。

import { h, replace } from '../../src/lib/dom.js';
import { filterModels } from './model.js';

/**
 * @param {any} dialog
 * @param {HTMLElement} anchor 在哪个按钮旁边
 * @param {{ref: string, name: string, model: string, provider: string}[]} rows 能选的
 * @param {(refs: string[]) => void} done 照勾的先后
 */
export function pickModels(dialog, anchor, rows, done) {
  const ctx = dialog.ctx;
  const t = (k, f) => ctx.text(`models.picker.${k}`, f);
  dialog.panel.querySelector('.set-menu')?.dispatchEvent(new CustomEvent('set-dismiss'));
  /** @type {string[]} */
  const picked = [];
  const search = /** @type {HTMLInputElement} */ (h('input.set-input', { type: 'search', placeholder: t('search'), spellcheck: 'false', autocomplete: 'off' }));
  const list = h('div.set-picker-list');
  const add = /** @type {HTMLButtonElement} */ (h('button.set-btn.is-primary', { type: 'button', disabled: true, onclick: () => { close(); done([...picked]); } }, t('add', { count: 0 })));
  const sync = () => {
    add.disabled = !picked.length;
    add.textContent = t('add', { count: picked.length });
  };
  const row = (r) => {
    const box = /** @type {HTMLInputElement} */ (h('input', { type: 'checkbox' }));
    box.addEventListener('change', () => {
      const i = picked.indexOf(r.ref);
      if (box.checked && i < 0) picked.push(r.ref);
      if (!box.checked && i >= 0) picked.splice(i, 1);
      sync();
    });
    return h('label.set-picker-row', box, h('span.set-model-name', r.name, r.name !== r.model ? h('code.set-model-id', r.model) : null), h('span.set-pick-provider', r.provider));
  };
  const nodes = new Map(rows.map((r) => [r, row(r)]));
  const draw = () => {
    const shown = filterModels(rows, search.value);
    replace(list, shown.length ? shown.map((r) => nodes.get(r)) : h('p.set-empty', t('none')));
  };
  search.addEventListener('input', draw);
  search.addEventListener('keydown', (e) => { if (e.key === 'Enter' && picked.length) add.click(); });
  const el = h('div.set-menu.set-picker', search, list, h('div.set-picker-foot', h('button.set-btn', { type: 'button', onclick: () => close() }, t('cancel')), add));
  const outside = (e) => { if (!el.contains(e.target) && !anchor.contains(e.target)) close(); };
  const close = () => {
    document.removeEventListener('pointerdown', outside, true);
    el.remove();
  };
  el.addEventListener('set-dismiss', close);
  // 照按钮的位置放，下面放不下往上开、右边放不下往左挪
  const box = dialog.panel.getBoundingClientRect();
  const at = anchor.getBoundingClientRect();
  const scale = box.width / dialog.panel.offsetWidth || 1;
  dialog.panel.append(el);
  draw();
  const w = el.offsetWidth;
  const left = Math.min((at.left - box.left) / scale, dialog.panel.offsetWidth - w - 12);
  el.style.left = `${Math.max(12, left)}px`;
  el.style.top = `${(at.bottom - box.top) / scale + 6}px`;
  if (el.getBoundingClientRect().bottom > box.bottom - 8) el.style.top = `${Math.max(12, (at.top - box.top) / scale - el.offsetHeight - 6)}px`;
  document.addEventListener('pointerdown', outside, true);
  search.focus();
  return el;
}

