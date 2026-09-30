// @ts-check
//! `@` 选文件的列表（蓝图 `web.md`「`@` 选文件」第 4、5 条，照 `tui.md`「`@` 文件列表」）：浮在输入框上面，和命令列表同一个位置、
//! 同一个样子（带 `dock-float`）。头一行「@ 打的字 · N 个」和第 2 条的那几句，一条一行（前面的目录那一截暗、名字原色，对上的字
//! 标出来），最下面一行暗色写按键。焦点留在输入框里（打的字照常进框里的词）：`↑` `↓` `Tab` `Enter` `Esc` 由输入框交过来
//! （`key`）。列、找由桥做，这里只画。

import { h, replace } from './dom.js';
import { show, hide } from '../lib/motion.js';
import { res, t } from '../util/res.js';

/** @typedef {{path: string, full: string, dir: boolean, size?: number|null, type?: string|null, marks: number[]}} Entry 桥交回的一条 */
/** @typedef {{word: string, items: Entry[], partial: boolean, layer: boolean, error?: string}} Found `error`：问不到，写为什么 */

export class MentionList {
  /** @param {{pick: (entry: Entry, how: 'enter'|'tab') => void, dismiss: () => void}} on 选定了一条；`Esc` 关掉 */
  constructor(on) {
    this.on = on;
    this.title = h('strong.mention-title');
    this.note = h('span.mention-note');
    this.list = h('div.mention-list', { role: 'listbox', style: `--rows: ${res.layout.command_rows}` });
    this.el = h('div.dock-mention.dock-float', { hidden: true, role: 'dialog' },
      h('div.mention-head', this.title, this.note), this.list, h('div.mention-hint', t('mention.hint')));
    /** @type {Entry[]} */
    this.items = [];
    this.selected = 0;
    this.pointer = '';
  }

  get open() { return !this.el.hidden && !this.el.classList.contains('is-leaving'); }

  /** 画一份找到的：打开时选中第一条。 @param {Found} found */
  show(found) {
    const keep = this.open ? this.items[this.selected]?.full : null;
    this.items = found.items;
    this.selected = Math.max(0, keep ? this.items.findIndex((x) => x.full === keep) : 0);
    this.title.textContent = t('mention.title', { word: found.word, count: found.items.length });
    this.note.textContent = found.partial ? t(found.layer ? 'mention.partial_layer' : 'mention.partial', { shown: found.items.length }) : found.layer ? t('mention.layer') : '';
    const empty = found.error ? h('div.mention-empty.is-error', found.error) : h('div.mention-empty', t('mention.no_match'));
    replace(this.list, this.items.length ? this.items.map((x, i) => this.row(x, i)) : [empty]);
    this.mark();
    show(this.el);
  }

  close() {
    if (this.open) hide(this.el);
  }

  /** 一条一行：前面的目录那一截暗、名字原色，对上的字标出来。 */
  row(entry, i) {
    const chars = Array.from(entry.path);
    const trimmed = entry.path.replace(/\/$/, '');
    const nameAt = Array.from(trimmed.slice(0, trimmed.lastIndexOf('/') + 1)).length;
    const marks = new Set(entry.marks);
    // 连着一样的切成一段：在不在目录那一截、是不是对上的字
    const parts = [];
    chars.forEach((c, k) => {
      const kind = `${k < nameAt ? 'dir' : 'name'}${marks.has(k) ? '.is-mark' : ''}`;
      const last = parts.at(-1);
      if (last && last.kind === kind) last.text += c;
      else parts.push({ kind, text: c });
    });
    return h('div.mention-row', {
      role: 'option',
      onmousedown: (/** @type {MouseEvent} */ e) => e.preventDefault(),
      onmousemove: (/** @type {MouseEvent} */ e) => this.hover(e, i),
      onclick: () => this.on.pick(entry, 'enter'),
    }, h('span.mention-path', parts.map((p) => h(`span.mention-${p.kind}`, p.text))));
  }

  mark() {
    const rows = /** @type {HTMLElement[]} */ ([...this.list.querySelectorAll('.mention-row')]);
    rows.forEach((r, j) => {
      r.classList.toggle('is-selected', j === this.selected);
      r.setAttribute('aria-selected', String(j === this.selected));
    });
    const row = rows[this.selected];
    if (!row) return;
    const top = row.offsetTop - this.list.offsetTop;
    const bottom = top + row.offsetHeight;
    if (top < this.list.scrollTop) this.list.scrollTop = top;
    else if (bottom > this.list.scrollTop + this.list.clientHeight) this.list.scrollTop = bottom - this.list.clientHeight;
  }

  hover(e, i) {
    const at = `${e.clientX},${e.clientY}`;
    const moved = at !== this.pointer;
    this.pointer = at;
    if (moved && i !== this.selected) {
      this.selected = i;
      this.mark();
    }
  }

  /** 输入框交过来的按键：`↑` `↓` 选，`Tab`、`Enter` 选定，`Esc` 关。接了的交回 `true`。 */
  key(e) {
    if (!this.open || e.ctrlKey || e.metaKey || e.altKey) return false;
    const n = this.items.length;
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      if (n) this.selected = (this.selected + (e.key === 'ArrowDown' ? 1 : n - 1)) % n;
      this.mark();
    } else if ((e.key === 'Tab' && !e.shiftKey) || (e.key === 'Enter' && !e.shiftKey)) {
      const entry = this.items[this.selected];
      if (!entry) return false;
      this.on.pick(entry, e.key === 'Tab' ? 'tab' : 'enter');
    } else if (e.key === 'Escape') {
      this.on.dismiss();
    } else {
      return false;
    }
    e.preventDefault();
    return true;
  }
}
