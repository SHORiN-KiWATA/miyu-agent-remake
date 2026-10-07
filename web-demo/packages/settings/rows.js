// @ts-check
//! 设置页的一行（蓝图 `web.md`「设置页」第 4–8 条）：左边名字、说明，右边控件；名字下面的小字只写和「默认、当场生效」不一样的；
//! 改了当场存，存的时候转圈、成了一个对勾、拒了放回原来的值并写原因；写过的悬停露 ↺ 恢复默认。核心的项（`coreRow`）和网页包的项
//! （`look.js`）共用 `shell`。

import { h, icon, replace } from '../../src/lib/dom.js';
import { noteOf, envRef, inputText, layerFor, writtenIn } from './model.js';

/**
 * 一组：组名，下面一行行。
 * @param {string} name
 * @param {Node[]} rows
 */
/** 一组：组名、几行；一行都没有的（专门的编辑器把行都藏了）整组不画。 */
export const groupBlock = (name, rows) => {
  const shown = rows.filter(Boolean);
  return shown.length ? h('section.set-group', h('h3.set-group-name', name), h('div.set-rows', shown)) : null;
};

/** 对不上哪一项的问题：那一页顶上一条横幅。 */
export const banner = (p) => h(`div.set-banner.is-${p.level === 'error' ? 'error' : 'warn'}`, icon(p.level === 'error' ? 'circle-alert' : 'triangle-alert'), h('span', p.message));

/**
 * 一行的壳：名字、说明、小字（没有就不出这一行）、问题；右边控件和 ↺。交回节点和几个钩子（转圈、对勾、写原因）。
 * @param {{name: string, description?: string, source?: Node|string|null, problems?: {level: string, message: string}[],
 *   control: Node, reset?: (() => void)|null, resetTitle?: string}} o
 */
export function shell(o) {
  const status = h('span.set-status');
  const error = h('p.set-error', { hidden: true });
  const el = h('div.set-row',
    h('div.set-text',
      h('div.set-name', h('span', o.name)),
      o.description ? h('p.set-desc', { title: o.description }, o.description) : null,
      o.source ? h('p.set-source', o.source) : null,
      ...(o.problems ?? []).map((p) => h(`p.set-problem.is-${p.level === 'error' ? 'error' : 'warn'}`, p.message)),
      error),
    h('div.set-control',
      o.reset ? h('button.set-reset', { type: 'button', title: o.resetTitle, 'aria-label': o.resetTitle, onclick: o.reset }, icon('rotate-ccw')) : null,
      status, o.control));
  return {
    el,
    /** 存着：转圈 */
    busy: () => { error.hidden = true; replace(status, icon('loader-circle')); status.className = 'set-status is-busy'; },
    /** 成了：对勾，1 秒淡出 */
    done: () => { replace(status, icon('check')); status.className = 'set-status is-done'; },
    /** 拒了：原因写在这一项下面 */
    fail: (why) => { replace(status); status.className = 'set-status'; error.textContent = why; error.hidden = false; },
  };
}

/**
 * 名字下面那一行小字（第 7 条）：值不是默认的写来自哪一层（悬停看文件和第几行，点了复制路径），不是当场生效的写什么时候生效；
 * 两样都没有的交 `null`。
 * @param {any} dialog
 * @param {import('./model.js').Item} item
 */
function sourceLine(dialog, item, compact) {
  const ctx = dialog.ctx;
  // 精简的（模型详情）：每一项都一样的生效时机不写，只写改过的来源
  const note = noteOf(item.entry, compact ? 'now' : item.applies);
  if (!note) return null;
  const parts = [];
  const src = note.source;
  if (src) {
    const where = src.file ? `${src.file}${src.line ? `:${src.line}` : ''}` : '';
    const layer = src.env ? ctx.text('env_layer', { name: src.env }) : ctx.text(`layers.${src.layer}`);
    parts.push(h(where ? 'button.set-origin' : 'span', where ? { type: 'button', title: where, onclick: () => copyPath(dialog, src.file ?? '') } : {}, layer));
  }
  if (note.applies) parts.push(parts.length ? ' · ' : '', ctx.text(`applies.${note.applies}`));
  return h('span', parts);
}

/** 点来源：桌面端以后用系统里的程序打开（宿主 `openPath`），浏览器复制路径（第 7 条）。 */
async function copyPath(dialog, file) {
  const host = dialog.ctx.host;
  if (typeof host.openPath === 'function' && (await host.openPath(file))) return;
  await host.clipboard?.write(file);
  dialog.toast(dialog.ctx.text('copied_path'));
}

/**
 * 核心的一项（第 4–8 条）。`compact` 的不写说明、不写生效时机（模型详情里：名字已经说清了，每一项都是下一轮生效）。
 * @param {any} dialog 设置页的弹窗（`dialog.js`）
 * @param {import('./model.js').Item} item
 * @param {{compact?: boolean}} [opts]
 */
export function coreRow(dialog, item, opts = {}) {
  const ctx = dialog.ctx;
  // 专门的编辑器（挂载位 `settings.editor`，照键）：别的包给这一项的选项（默认人格照人格列表），也可以把这一行藏了（没得选的）
  const editor = ctx.slots.pick('settings.editor', item.key);
  if (editor?.hidden?.()) return null;
  const value = item.entry?.value ?? item.fact?.value;
  /** @type {ReturnType<typeof shell>} */
  let row;
  const save = async (change) => {
    row.busy();
    const why = await dialog.save(item, change);
    if (why) row.fail(why);
    else row.done();
  };
  const written = writtenIn(item.entry, layerFor(item));
  row = shell({
    name: item.name,
    description: opts.compact ? '' : item.description,
    source: sourceLine(dialog, item, opts.compact),
    problems: item.problems,
    control: editor?.options ? control(dialog, { ...item, control: 'select', options: editor.options() }, value, save) : control(dialog, item, value, save),
    reset: written ? () => save({ unset: true }) : null,
    resetTitle: ctx.text('reset'),
  });
  return row.el;
}

/**
 * 控件（第 5 条），照 `control`。
 * @param {any} dialog
 * @param {import('./model.js').Item} item
 * @param {any} value 最终值
 * @param {(change: {value?: any, input?: string}) => void} save
 */
function control(dialog, item, value, save) {
  const ctx = dialog.ctx;
  if (item.control === 'toggle') return toggle(!!value, (on) => save({ value: on }));
  if (item.control === 'select') {
    const options = (item.options ?? []).map((o) => ({ value: o.value, name: o.name, note: o.value === item.default ? ctx.text('default_mark') : '' }));
    // 选了值是 `null` 的那一项（「默认」）：从这一层删掉
    return select(dialog, options, value ?? null, (v) => save(v === null ? { unset: true } : { value: v }));
  }
  if (item.control === 'list') return list(dialog, item, Array.isArray(value) ? value : [], (next) => save({ value: next }));
  // 文字、数：一行字，`Enter`、离开时存，整串交给核心照类型读（`input`）；清空了是从这一层删掉（回到默认）；环境变量引用的先写
  // 「来自环境变量 X」，点了才改
  const env = envRef(value);
  const field = textField(inputText(env ? '' : value), item.control === 'number' ? 'number' : 'text', item.max != null && item.min != null ? `${item.min} – ${item.max}` : '', (text) => save(text === '' ? { unset: true } : { input: text }));
  if (!env) return field;
  const label = h('button.set-env', { type: 'button', onclick: () => { label.replaceWith(field); field.focus(); } }, ctx.text('from_env', { name: env }));
  return label;
}

/** 开关。 */
export function toggle(on, change) {
  return h(`button.set-switch${on ? '.is-on' : ''}`, { type: 'button', role: 'switch', 'aria-checked': String(on), onclick: () => change(!on) }, h('i'));
}

/**
 * 一行字：改了打个记号（别处改了不冲掉它，`dialog.editing`），`Enter`、离开时字变了才存，`Esc` 放回原来的。
 * @param {string} text @param {'text'|'number'} kind @param {string} hint 悬停写范围 @param {(text: string) => void} commit
 */
export function textField(text, kind, hint, commit) {
  const input = /** @type {HTMLInputElement} */ (h(`input.set-input.is-${kind}`, { type: 'text', value: text, title: hint || null, spellcheck: 'false', inputmode: kind === 'number' ? 'decimal' : null }));
  input.addEventListener('input', () => { input.dataset.dirty = '1'; });
  const done = () => {
    if (input.dataset.dirty !== '1') return;
    input.dataset.dirty = '';
    if (input.value !== text) commit(input.value.trim());
  };
  input.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      done();
    }
    if (e.key === 'Escape' && input.dataset.dirty === '1') {
      e.stopPropagation();
      input.value = text;
      input.dataset.dirty = '';
    }
  });
  input.addEventListener('blur', done);
  return input;
}

/**
 * 下拉：按钮写选中那一项，点开一个菜单（照「菜单」的浮层），默认那一项后面暗色写「默认」。
 * @param {any} dialog @param {{value: any, name: string, note?: string}[]} options @param {any} value @param {(v: any) => void} pick
 */
export function select(dialog, options, value, pick) {
  const chosen = options.find((o) => JSON.stringify(o.value) === JSON.stringify(value));
  const button = h('button.set-select', { type: 'button', onclick: () => menu(dialog, button, options, value, pick) }, h('span', chosen?.name ?? inputText(value)), icon('chevron-down'));
  return button;
}

/** 在 `anchor` 下面开一个菜单；点外面、`Esc`、选了一项关上。 */
export function menu(dialog, anchor, options, value, pick) {
  dialog.panel.querySelector('.set-menu')?.dispatchEvent(new CustomEvent('set-dismiss'));
  const el = h('div.set-menu', { role: 'listbox' }, options.map((o) => h(`button.set-menu-item${JSON.stringify(o.value) === JSON.stringify(value) ? '.is-on' : ''}`, {
    type: 'button',
    role: 'option',
    onclick: () => { close(); if (JSON.stringify(o.value) !== JSON.stringify(value)) pick(o.value); },
  }, h('span', o.name), o.note ? h('em', o.note) : null, JSON.stringify(o.value) === JSON.stringify(value) ? icon('check') : null)));
  const outside = (e) => { if (!el.contains(e.target) && e.target !== anchor && !anchor.contains(e.target)) close(); };
  const close = () => {
    document.removeEventListener('pointerdown', outside, true);
    el.remove();
  };
  el.addEventListener('set-dismiss', close);
  const box = dialog.panel.getBoundingClientRect();
  const at = anchor.getBoundingClientRect();
  const scale = box.width / dialog.panel.offsetWidth || 1;
  el.style.right = `${(box.right - at.right) / scale}px`;
  el.style.top = `${(at.bottom - box.top) / scale + 4}px`;
  dialog.panel.append(el);
  // 下面放不下往上开
  if (el.getBoundingClientRect().bottom > box.bottom - 8) el.style.top = `${(at.top - box.top) / scale - el.offsetHeight - 4}px`;
  document.addEventListener('pointerdown', outside, true);
  return el;
}

/**
 * 列表：一个个小块，每块 ✕，最后一个「＋」（元素是选项的开菜单，别的开一个小框）。
 * @param {any} dialog @param {import('./model.js').Item} item @param {any[]} values @param {(next: any[]) => void} change
 */
function list(dialog, item, values, change) {
  const ctx = dialog.ctx;
  const nameOf = (v) => item.options?.find((o) => o.value === v)?.name ?? inputText(v);
  const chips = values.map((v, i) => h('span.set-chip', h('span', nameOf(v)), h('button', { type: 'button', 'aria-label': '×', onclick: () => change(values.filter((_, n) => n !== i)) }, icon('x'))));
  const add = h('button.set-chip.is-add', { type: 'button', title: ctx.text('add'), 'aria-label': ctx.text('add') }, icon('plus'));
  add.addEventListener('click', () => {
    if (item.options?.length) {
      const rest = item.options.filter((o) => !values.includes(o.value)).map((o) => ({ value: o.value, name: o.name }));
      if (rest.length) menu(dialog, add, rest, null, (v) => change([...values, v]));
      return;
    }
    const field = textField('', 'text', '', (text) => { if (text) change([...values, text]); });
    add.replaceWith(field);
    field.focus();
  });
  return h('div.set-chips', chips, add);
}
