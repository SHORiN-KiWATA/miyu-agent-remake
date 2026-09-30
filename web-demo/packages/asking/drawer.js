// @ts-check
//! 确认和提问的抽屉（蓝图 `web.md`「确认和提问」第 2–5 条）：画在输入框里（挂载位 `composer.takeover`），照 `model.js` 的抽屉画；
//! 按键、鼠标交给 `model.js` 算新的抽屉，交了、取消了交给 `index.js`。编辑（「输入其他答案」、补充、不允许的理由）用一个
//! 跟着字长高的框：`Enter` 保存、`Shift+Enter` 换行、`Esc` 退出编辑。

import { h, replace } from '../../src/lib/dom.js';
import { press, saveEdit, cancel, hasReview, onOther, answerOf, approvalHead } from './model.js';
import { isNewline, insertNewline } from '../../src/lib/newline.js';

/** 键 → `model.js` 的按键名 */
const KEYS = { ArrowUp: 'up', ArrowDown: 'down', ArrowLeft: 'left', ArrowRight: 'right', Tab: 'tab', ' ': 'space', Enter: 'enter', n: 'n', N: 'n' };

export class Drawer {
  /**
   * @param {{max_vh: number, preview_min_width: number, esc_window_ms: number}} config
   * @param {(key: string, fields?: Record<string, any>) => string} text
   * @param {(result: import('./model.js').Result, d: import('./model.js').Drawer) => void} settle 交了、取消了
   * @param {() => string|null} home 家目录：确认里的路径写成 `~`
   */
  constructor(config, text, settle, home) {
    this.config = config;
    this.text = text;
    this.settle = settle;
    this.home = home;
    /** @type {import('./model.js').Drawer|null} */
    this.d = null;
    /** @type {'other'|'note'|'reason'|null} 在编辑哪一样 */
    this.editing = null;
    /** 按过一下 `Esc` 的时刻（两下取消） */
    this.escAt = 0;
    this.pointer = '';
    this.el = h('div.asking', { hidden: true, tabindex: '-1', role: 'dialog', style: `--asking-max-vh: ${config.max_vh}` });
    this.el.addEventListener('keydown', (e) => this.key(e));
    // 够宽时文字画放在选项右边
    new ResizeObserver(() => this.el.classList.toggle('is-wide', this.el.clientWidth >= config.preview_min_width)).observe(this.el);
  }

  get open() { return this.d != null; }

  /** 打开一个：焦点进抽屉。 @param {import('./model.js').Drawer} d */
  show(d) {
    this.d = d;
    this.editing = null;
    this.escAt = 0;
    this.el.hidden = false;
    this.draw();
    this.el.focus({ preventScroll: true });
  }

  /** 焦点进抽屉（露出来以后才给得上）。 */
  focus() {
    this.el.focus({ preventScroll: true });
  }

  close() {
    this.d = null;
    this.editing = null;
    this.el.hidden = true;
    replace(this.el);
  }

  /** @param {import('./model.js').Step} step */
  step(step) {
    if (step.done) {
      this.finish(step.done);
      return;
    }
    this.d = step.d;
    this.editing = step.edit;
    this.draw();
  }

  /** @param {KeyboardEvent} e */
  key(e) {
    if (!this.d || e.isComposing || e.keyCode === 229) return;
    const inBox = /** @type {HTMLElement} */ (e.target).tagName === 'TEXTAREA';
    if (inBox) {
      // Ctrl+J 换行（照 TUI，和输入框一样；Shift+Enter 由框自己换）
      if (e.ctrlKey && isNewline(e)) {
        e.preventDefault();
        insertNewline(/** @type {HTMLTextAreaElement} */ (e.target));
        return;
      }
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault();
        this.save(/** @type {HTMLTextAreaElement} */ (e.target).value);
      } else if (e.key === 'Escape') {
        e.preventDefault();
        this.editing = null;
        this.draw();
        this.el.focus({ preventScroll: true });
      }
      return;
    }
    if (e.ctrlKey || e.metaKey || e.altKey) return;
    if (e.key === 'Escape') {
      e.preventDefault();
      if (Date.now() - this.escAt < this.config.esc_window_ms) return this.cancel();
      this.escAt = Date.now();
      this.drawKeys();
      return;
    }
    const name = e.shiftKey && e.key === 'Tab' ? 'left' : /^[1-9]$/.test(e.key) ? e.key : KEYS[/** @type {keyof KEYS} */ (e.key)];
    if (!name || (e.shiftKey && e.key !== 'Tab')) return;
    e.preventDefault();
    this.escAt = 0;
    this.step(press(this.d, name));
  }

  /** 编辑的框里按了 `Enter`。 @param {string} value */
  save(value) {
    if (!this.d || !this.editing) return;
    const what = this.editing;
    this.editing = null;
    this.step(saveEdit(this.d, value, what));
    if (this.open) this.el.focus({ preventScroll: true });
  }

  /** 两下 `Esc`、点「取消」。 */
  cancel() {
    if (this.d) this.finish(cancel(this.d));
  }

  /**
   * 了结：先交出去（`index.js` 先把框还回来：这时抽屉还画着，框照抽屉的高度缓回去，不先塌成一条），再清掉抽屉；
   * 交出去的时候排着的下一个已经打开了的，不清。
   * @param {import('./model.js').Result} result
   */
  finish(result) {
    const d = /** @type {import('./model.js').Drawer} */ (this.d);
    this.d = null;
    this.editing = null;
    this.settle(result, d);
    if (!this.d) this.close();
  }

  draw() {
    const d = /** @type {import('./model.js').Drawer} */ (this.d);
    const t = this.text;
    const review = d.tab === d.questions.length;
    const scroll = h('div.asking-body', review ? this.reviewPage() : this.questionPage());
    replace(this.el,
      d.who ? h('div.asking-who', t('asking', { who: d.who })) : null,
      hasReview(d) ? this.tabs() : null,
      scroll,
      this.foot = h('div.asking-foot', this.keysEl = h('span.asking-keys'), h('button.asking-cancel', { type: 'button', onclick: () => this.cancel() }, t('cancel'))));
    this.drawKeys();
    scroll.querySelector('.asking-option.is-selected')?.scrollIntoView({ block: 'nearest' });
    const box = /** @type {HTMLTextAreaElement|null} */ (this.el.querySelector('textarea'));
    if (box) {
      box.focus();
      box.setSelectionRange(box.value.length, box.value.length);
      grow(box);
    }
  }

  /** 两道题及以上：一排标签，最后一个是「确认」。 */
  tabs() {
    const d = /** @type {import('./model.js').Drawer} */ (this.d);
    const names = [...d.questions.map((q, i) => `${q.header || i + 1}${answerOf(d, i) ? ' ✓' : ''}`), this.text('review_tab')];
    return h('div.asking-tabs', names.map((name, i) => h(`button.asking-tab${i === d.tab ? '.is-current' : ''}`, {
      type: 'button', onclick: () => this.step({ d: { ...d, tab: i }, done: null, edit: null }),
    }, name)));
  }

  questionPage() {
    const d = /** @type {import('./model.js').Drawer} */ (this.d);
    const t = this.text;
    const q = d.questions[d.tab];
    const head = d.kind === 'approve' ? approvalHead(d.body, this.home(), t) : null;
    const options = q.options.map((o, i) => this.option(i, d.kind === 'approve' ? t(`decision.${o.decision}`) : o.label, o.description));
    if (d.kind === 'ask') options.push(this.otherRow());
    const previews = q.options.some((o) => o.preview)
      ? h('div.asking-previews', q.options.map((o, i) => h(`pre.asking-preview${i === d.cursor[d.tab] ? '.is-shown' : ''}`, o.preview ?? '')))
      : null;
    return [
      h('div.asking-question', head ? head.title : q.question),
      head?.paths.length ? h('div.asking-paths', head.paths.map((p) => h('div.asking-path', h('span', p.path), p.outside ? h('span.asking-outside', t('outside')) : null))) : null,
      h(`div.asking-main${previews ? '.has-preview' : ''}`, h('div.asking-options', options), previews),
      d.kind === 'ask' ? this.notesRow() : null,
    ];
  }

  /** 一个选项：编号（能多选的是勾选框）、标题、说明；确认的「不允许」在编辑时下面接理由的框。 */
  option(i, label, description) {
    const d = /** @type {import('./model.js').Drawer} */ (this.d);
    const q = d.questions[d.tab];
    const selected = d.cursor[d.tab] === i;
    const picked = q.multiple ? d.checked[d.tab].has(i) : d.choice[d.tab] === i;
    const mark = q.multiple ? h(`span.asking-check${picked ? '.is-on' : ''}`, picked ? '✓' : '') : h('span.asking-num', String(i + 1));
    const reason = d.kind === 'approve' && selected && this.editing === 'reason' ? this.editBox(this.text('reason_hint'), '') : null;
    return h(`div.asking-option${selected ? '.is-selected' : ''}${picked ? '.is-picked' : ''}`, this.pointerProps(i),
      mark, h('div.asking-text', h('strong', label), description ? h('span.asking-desc', description) : null, reason));
  }

  /** 最后一项「输入其他答案」：编辑时是一个框，保存了的写成「自定义：打的字」。 */
  otherRow() {
    const d = /** @type {import('./model.js').Drawer} */ (this.d);
    const i = d.questions[d.tab].options.length;
    const selected = onOther(d);
    const saved = d.custom[d.tab];
    const body = this.editing === 'other' && selected
      ? this.editBox(this.text('edit_hint'), saved)
      : h('strong', saved ? this.text('custom', { text: saved }) : this.text('other'));
    return h(`div.asking-option.is-other${selected ? '.is-selected' : ''}`, this.pointerProps(i), h('span.asking-num', String(i + 1)), h('div.asking-text', body));
  }

  /** 补充：写了的一行 `补充  打的字`，编辑时是一个框；没写的一个小按钮。 */
  notesRow() {
    const d = /** @type {import('./model.js').Drawer} */ (this.d);
    const t = this.text;
    if (this.editing === 'note') return h('div.asking-notes', h('span.asking-notes-label', t('notes_label')), this.editBox(t('edit_hint'), d.notes[d.tab]));
    if (d.notes[d.tab]) return h('div.asking-notes', h('span.asking-notes-label', t('notes_label')), h('span', d.notes[d.tab]));
    return h('button.asking-notes-add', { type: 'button', onclick: () => this.step({ d, done: null, edit: 'note' }) }, t('notes_add'));
  }

  /** 「确认」页：一道一行 `短名：回答`，没答的红色「未回答」；下面「提交」。 */
  reviewPage() {
    const d = /** @type {import('./model.js').Drawer} */ (this.d);
    return [
      h('div.asking-review', d.questions.map((q, i) => {
        const got = answerOf(d, i);
        return h('div.asking-review-row', h('strong', `${q.header || q.question}：`), got ? h('span', got) : h('span.is-missing', this.text('unanswered')));
      })),
      h('button.asking-submit', { type: 'button', onclick: () => this.step(press(d, 'enter')) }, this.text('submit')),
    ];
  }

  /** 悬停选中、点一下等于 `Enter`（指针真的动了才换，滚动带过来的不算）。 */
  pointerProps(i) {
    return {
      onmousemove: (/** @type {MouseEvent} */ e) => {
        const at = `${e.clientX},${e.clientY}`;
        if (at === this.pointer || !this.d || this.editing) return;
        this.pointer = at;
        if (this.d.cursor[this.d.tab] !== i) this.step({ d: { ...this.d, cursor: this.d.cursor.map((c, j) => (j === this.d?.tab ? i : c)) }, done: null, edit: null });
      },
      onclick: (/** @type {MouseEvent} */ e) => {
        if (!this.d || /** @type {HTMLElement} */ (e.target).tagName === 'TEXTAREA') return;
        const moved = { ...this.d, cursor: this.d.cursor.map((c, j) => (j === this.d?.tab ? i : c)) };
        const q = moved.questions[moved.tab];
        this.step(press(moved, q.multiple && i < q.options.length ? 'space' : 'enter'));
      },
    };
  }

  /** 编辑的框：跟着字长高。 @param {string} hint @param {string} value */
  editBox(hint, value) {
    const box = /** @type {HTMLTextAreaElement} */ (h('textarea.asking-edit', { rows: 1, placeholder: hint, spellcheck: 'false' }));
    box.value = value;
    box.addEventListener('input', () => grow(box));
    return box;
  }

  /** 最下面一行：照这一页能做什么写；按过一下 `Esc` 换成「再按一次 Esc 取消」。 */
  drawKeys() {
    const d = this.d;
    if (!d || !this.keysEl) return;
    const t = this.text;
    const armed = this.escAt && Date.now() - this.escAt < this.config.esc_window_ms;
    this.keysEl.classList.toggle('is-armed', !!armed);
    if (armed) {
      this.keysEl.textContent = t('cancel_hint');
      clearTimeout(this.escTimer);
      this.escTimer = setTimeout(() => { this.escAt = 0; this.drawKeys(); }, this.config.esc_window_ms);
      return;
    }
    const q = d.questions[d.tab];
    const base = this.editing ? 'keys_edit'
      : d.tab === d.questions.length ? 'keys_review'
        : d.kind === 'approve' ? 'keys_approve'
          : q.multiple ? 'keys_multi' : 'keys';
    this.keysEl.textContent = t(base) + (hasReview(d) && !this.editing ? t('keys_tabs') : '');
  }
}

/** 框跟着字长高。 @param {HTMLTextAreaElement} box */
function grow(box) {
  box.style.height = 'auto';
  box.style.height = `${box.scrollHeight}px`;
}

