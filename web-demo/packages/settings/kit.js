// @ts-check
//! 交给别的包那一页的控件（挂载位 `settings.section` 的 `render(kit)`，蓝图 `web/architecture.md`「设置页」）：别的包画的页和设置页
//! 长一个样，控件不各抄一份。`text` 是改完就存的一行字（`Enter`、离开时），`field` 是只记着、等那一页自己存的一行字；输入时打记号，
//! 别处改了配置重画时不冲掉正在写的（`dialog.editing`）。页里带 `data-set-dismiss` 的那一块，`Esc` 先发给它一个 `set-dismiss`
//! （它自己定收不收），没有的才关弹窗。

import { h } from '../../src/lib/dom.js';
import { toggle, textField, select } from './rows.js';

/**
 * @typedef {{
 *   toggle: (on: boolean, change: (on: boolean) => void) => HTMLElement,
 *   select: (options: {value: any, name: string, note?: string}[], value: any, pick: (v: any) => void) => HTMLElement,
 *   text: (text: string, hint: string, commit: (text: string) => void) => HTMLInputElement,
 *   field: (text: string, hint: string) => HTMLInputElement,
 *   button: (label: string, opts: {primary?: boolean, danger?: boolean}, onclick: () => void) => HTMLButtonElement,
 *   toast: (text: string) => void,
 * }} Kit
 */

/** @param {any} dialog @returns {Kit} */
export function sectionKit(dialog) {
  return {
    toggle,
    select: (options, value, pick) => select(dialog, options, value, pick),
    text: (text, hint, commit) => textField(text, 'text', hint, commit),
    field: (text, hint) => {
      const input = /** @type {HTMLInputElement} */ (h('input.set-input.is-text', { type: 'text', value: text, placeholder: hint || null, spellcheck: 'false', autocomplete: 'off' }));
      input.addEventListener('input', () => { input.dataset.dirty = '1'; });
      return input;
    },
    button: (label, opts, onclick) => /** @type {HTMLButtonElement} */ (h(`button.set-btn${opts.primary ? '.is-primary' : ''}${opts.danger ? '.is-danger' : ''}`, { type: 'button', onclick }, label)),
    toast: (text) => dialog.toast(text),
  };
}
