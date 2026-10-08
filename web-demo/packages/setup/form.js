// @ts-check
//! 人格、预设编辑器共用的几样（软件包 `setup`，蓝图 `web.md`「人格、预设、工作区」第 6 条）：一段的小标题、一行名字和控件、跟着字长高的
//! 多行框（空着时占位字说写什么，不另加说明行）、点两次才做的按钮（删除、恢复出厂，不弹窗）、新建时只填名字的那一块。控件照设置页给的
//! `kit`（`settings/kit.js`），和设置页长一个样。

import { h, icon } from '../../src/lib/dom.js';

/**
 * 设置页交来的控件（`settings.section` 的 `render(kit)`；软件包之间不互相 import，这里照用到的写一份形状）。
 * @typedef {{
 *   toggle: (on: boolean, change: (on: boolean) => void) => HTMLElement,
 *   select: (options: {value: any, name: string, note?: string}[], value: any, pick: (v: any) => void) => HTMLElement,
 *   text: (text: string, hint: string, commit: (text: string) => void) => HTMLInputElement,
 *   field: (text: string, hint: string) => HTMLInputElement,
 *   button: (label: string, opts: {primary?: boolean, danger?: boolean}, onclick: () => void) => HTMLButtonElement,
 *   toast: (text: string) => void,
 * }} Kit
 */

/** 一段：暗色小标题，下面是内容。 @param {string} title @param {...any} kids */
export const part = (title, ...kids) => h('div.setup-part', h('h5.setup-part-name', title), ...kids);

/** 一行：左边名字（下面可以有一行暗色的说明），右边控件。 @param {string} name @param {string|null} desc @param {HTMLElement} control */
export const row = (name, desc, control) => h('div.setup-row', h('div.setup-row-text', h('span.setup-row-name', name), desc ? h('span.setup-row-desc', desc) : null), control);

/** 编辑器的头：名字，右边「收起」。 @param {HTMLElement} title @param {string} label @param {() => void} close */
export const head = (title, label, close) => h('div.setup-head', title, h('button.setup-collapse', { type: 'button', onclick: close }, label, icon('chevron-down')));

/**
 * 多行框：跟着字长高；输入时打记号（设置页重画时不冲掉正在写的，`settings` 的 `editing`）。
 * @param {string} text @param {string} hint 空着时的占位字 @param {number} [rows] 最少几行
 */
export function area(text, hint, rows = 3) {
  const el = /** @type {HTMLTextAreaElement} */ (h('textarea.setup-area', { rows, placeholder: hint, spellcheck: 'false' }));
  el.value = text;
  const grow = () => {
    el.style.height = 'auto';
    el.style.height = `${el.scrollHeight}px`;
  };
  el.addEventListener('input', () => {
    el.dataset.dirty = '1';
    grow();
  });
  // 放进页面以后才量得出高
  requestAnimationFrame(grow);
  return el;
}

/**
 * 点两次才做的按钮：第一次变成「再点一次…」，3 秒没再点变回去（2026-10-08 项目主人照推荐定：不弹窗）。
 * @param {Kit} kit @param {string} label @param {string} again @param {() => void} run
 */
export function twoClick(kit, label, again, run) {
  let armed = false;
  let timer = 0;
  const reset = () => {
    armed = false;
    clearTimeout(timer);
    btn.textContent = label;
    btn.classList.remove('is-armed');
  };
  const btn = kit.button(label, { danger: true }, () => {
    if (armed) {
      reset();
      run();
      return;
    }
    armed = true;
    btn.textContent = again;
    btn.classList.add('is-armed');
    timer = setTimeout(reset, 3000);
  });
  return btn;
}

/**
 * 新建：只填名字的一块（2026-10-08 项目主人：人格先填名字，建好接着在详情里写；预设填名字，功能先全开）。名字空着「建好」点不了；
 * `create` 交回一句错就写在下面、字留着。`Esc`（设置页的 `set-dismiss`）和「取消」收起。
 * @param {Kit} kit @param {(key: string) => string} t @param {string} title @param {string} hint
 * @param {(name: string) => Promise<string|null>} create @param {() => void} cancel
 */
export function nameFirst(kit, t, title, hint, create, cancel) {
  const field = kit.field('', hint);
  const err = h('p.setup-error', { hidden: true });
  const submit = async () => {
    const name = field.value.trim();
    if (!name) return;
    ok.disabled = true;
    const why = await create(name);
    err.hidden = !why;
    err.textContent = why ?? '';
    ok.disabled = false;
  };
  const ok = kit.button(t('edit.create'), { primary: true }, submit);
  ok.disabled = true;
  field.addEventListener('input', () => { ok.disabled = !field.value.trim(); });
  field.addEventListener('keydown', (e) => {
    if (e.key !== 'Enter' || e.isComposing) return;
    e.preventDefault();
    submit();
  });
  const el = h('div.setup-card.is-open', { 'data-set-dismiss': '' },
    h('div.setup-head', h('h4', title)),
    row(t('edit.name'), null, field),
    err,
    h('div.setup-foot', h('span.setup-grow'), kit.button(t('edit.cancel'), {}, cancel), ok));
  el.addEventListener('set-dismiss', cancel);
  requestAnimationFrame(() => field.focus());
  return el;
}
