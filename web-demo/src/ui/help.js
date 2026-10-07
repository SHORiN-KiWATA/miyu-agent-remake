// @ts-check
//! 帮助（`/help`，蓝图 `web.md`「帮助」，照 `tui.md`「帮助 `/help`」）：浮在输入框上面，和命令列表、选语言的浮层同一个位置、同一个样子，
//! 进出照「动效」。头一行标题、右边暗色写按键；下面两段：命令（现在的全部斜杠命令，出厂的加软件包登记的，名字一列对齐）、
//! 按键（`text/*.json` 的 `help.keys`）。放不下时自己滚：`↑` `↓` 一行一行、`PgUp` `PgDn` 一页一页，滚轮照常。`Esc`、点外面关，
//! 接着打字也关（输入框的字一变）。
//! 开着时这几个键在整页最先接走（焦点还在输入框里）。

import { h, replace } from './dom.js';
import { show, hide } from '../lib/motion.js';
import { res, t } from '../util/res.js';
import { fitAbove } from './fit.js';

/** 滚一行多高（和一行的行高一样） */
const LINE = 24;

export class HelpPanel {
  constructor() {
    this.body = h('div.help-body');
    this.el = h('div.dock-help.dock-float', { hidden: true, role: 'dialog' },
      h('div.picker-head', h('strong.picker-title', t('help.title')), h('span.picker-hint', t('help.hint'))), this.body);
    this.isOpen = false;
    this.onKey = (/** @type {KeyboardEvent} */ e) => {
      const page = Math.max(LINE, this.body.clientHeight - LINE);
      const by = { ArrowDown: LINE, ArrowUp: -LINE, PageDown: page, PageUp: -page }[e.key];
      if (by) this.body.scrollBy({ top: by });
      else if (e.key === 'Escape') this.close();
      // 接着打字的照常进输入框，框里的字一变帮助就让开（输入框那边收，输入法打的也算）
      else return;
      e.preventDefault();
      e.stopPropagation();
    };
    this.onDown = (/** @type {PointerEvent} */ e) => {
      if (!this.el.contains(/** @type {Node} */ (e.target))) this.close();
    };
  }

  get open() { return this.isOpen; }

  /** @param {import('../model/commands.js').Spec[]} specs 现在的全部斜杠命令 */
  show(specs) {
    const section = (title, rows) => [h('div.help-section', title), h('div.help-grid', rows)];
    const commands = specs.flatMap((s) => [
      h('code.help-key', [`/${s.name}`, ...(s.aliases ?? []).map((a) => `/${a}`)].join(' · ')),
      h('span.help-what', s.summary),
    ]);
    const keys = /** @type {[string, string][]} */ (res.text.help.keys).flatMap(([key, what]) => [h('kbd.help-key', key), h('span.help-what', what)]);
    replace(this.body, section(t('help.commands'), commands), section(t('help.keys_title'), keys));
    this.body.scrollTop = 0;
    this.isOpen = true;
    show(this.el);
    // 输入框在窗口中间时（空会话）放不下：框里矮下去，顶上留出吉祥物站的地方（`fit.js`）
    fitAbove(this.el, this.body, LINE * 3, false);
    document.addEventListener('keydown', this.onKey, true);
    document.addEventListener('pointerdown', this.onDown, true);
  }

  close() {
    if (!this.isOpen) return;
    this.isOpen = false;
    document.removeEventListener('keydown', this.onKey, true);
    document.removeEventListener('pointerdown', this.onDown, true);
    hide(this.el);
  }
}
