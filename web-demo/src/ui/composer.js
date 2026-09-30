// @ts-check
//! 输入框这一块（蓝图 `web.md`「输入框」「输入框下面那一行」「提示」「按键」「斜杠命令」）：圆角照 Claude 网页端的输入框，
//! 下面一行照 TUI 的「框下面那一行」，提示照 TUI 的「提示」。
//!
//! 按键照 `tui.md`「按键」：`Enter` 发，`Shift+Enter` 换行，输入法在选字时不算；两下 `Esc` 清空（1.5 秒内）。
//! 框里是斜杠命令的，回车执行它，不发给她（`model/commands.js` 的 `read`）；命令列表开着时 `↑` `↓` `Tab` `Enter` `Esc`
//! 先归列表（`ui/commands.js`）。焦点不在哪个输入框里时按 `/`，只把焦点放回这里，不打进这个 `/`。
//!
//! 框（`box`）上面浮着的几样挂在框上：提示、命令列表、运行状态行（`ui/pulse.js`，整页挂）；待办在框上面的流里
//! （`ui/todo.js`，整页挂）。在回答时整块带 `is-running`：提示浮到运行状态行上面，待办下面给它空出地方（`styles/dock.css`）。
//!
//! 框里写字的地方上面一排（`head`）、下面一排左边（`tools`）由整页挂软件包画的东西（附件，挂载位 `composer.head`、
//! `composer.bar`）；跟着话一起发的（`payload`，挂载位 `composer.payload`）发的时候交出来，拒了放回去（蓝图「附件」第 4 条）。
//! 框下面那一行的中间（`middle`，挂载位 `composer.footer`：后台任务的按钮）、浮在框上面的（`float`，挂载位 `composer.float`：
//! 后台任务的浮层，和命令列表同一个位置）也由整页挂。

import { h, icon, replace } from './dom.js';
import { res, t } from '../util/res.js';
import { fit } from '../model/footer.js';
import { read } from '../model/commands.js';
import { CommandList } from './commands.js';
import { show, hide, span } from '../lib/motion.js';

/**
 * @typedef {{has: () => boolean, busy: () => boolean, take: () => Record<string, any>|null, putBack: (given: any) => void}} Payload
 *   跟着话一起发的一样（挂载位 `composer.payload` 的一件，蓝图 `web/architecture.md`）
 */

export class Composer {
  /**
   * @param {{send: (text: string, extra: Record<string, any>) => Promise<boolean>, interrupt: () => void, cycleLevel: () => void,
   *   command: (spec: import('../model/commands.js').Spec, words: string|null) => void}} on `send` 交回核心收没收
   * @param {() => import('../model/commands.js').Spec[]} specs 现在的全部斜杠命令（出厂的加软件包登记的）
   * @param {() => Payload[]} payload 现在跟着话一起发的几样（挂载位 `composer.payload`）
   */
  constructor(on, specs, payload) {
    this.on = on;
    this.specs = specs;
    this.payload = payload;
    this.running = false;
    this.esc = 0;
    this.noticeTimer = 0;
    this.input = /** @type {HTMLTextAreaElement} */ (h('textarea.composer-input', {
      rows: 1,
      placeholder: t('placeholder', { name: res.persona.name }),
      onkeydown: (ev) => this.key(ev),
      oninput: () => this.changed(),
    }));
    this.sendButton = h('button.composer-send', { type: 'button', title: t('send'), onclick: () => this.submit() }, icon('arrow-up'));
    this.notice = h('div.composer-notice', { hidden: true });
    /** 框下面左边：级别那一格一直是这一个按钮（换级别时字卷上去、换新的，见 `drawLevel`），后面是模型 */
    this.levelRoll = h('span.level-roll');
    this.levelButton = h('button.footer-level', { type: 'button', title: t('level_tip'), onclick: () => this.on.cycleLevel() }, this.levelRoll);
    this.drawnLevel = /** @type {string|null} */ (null);
    /** 级别那一格的宽度正在缓的那一段（快速连按时停掉上一次的） */
    this.levelWidth = /** @type {Animation|null} */ (null);
    this.model = h('span.footer-model');
    this.left = h('span.footer-left', this.levelButton, this.model);
    this.right = h('span.footer-right');
    this.middle = h('span.footer-middle');
    this.footer = h('div.composer-footer', this.left, this.middle, this.right);
    this.menu = new CommandList({ run: (spec) => this.run(spec, null), fill: (text) => this.fill(text) }, specs);
    this.head = h('div.composer-head');
    this.tools = h('span.composer-tools');
    this.float = h('div.composer-float');
    this.box = h('div.composer', this.notice, this.head, this.input, this.bar = h('div.composer-bar', this.tools, this.sendButton), this.menu.el, this.float);
    this.el = h('div.composer-dock', this.box, this.footer);
    this.parts = /** @type {{key: string, text: string}[]} */ ([]);
    /** 撤销时放回框里的那句：恢复时还没动过的收回去（`tui.md`「输入框」第 7 条）。 */
    this.putBackText = /** @type {string|null} */ (null);
    // 那一行变宽变窄、中间的按钮出现消失：右边重新挑放得下的
    const fit = new ResizeObserver(() => this.drawRight());
    fit.observe(this.footer);
    fit.observe(this.middle);
    document.addEventListener('keydown', (ev) => this.slash(ev));
  }

  focus() { this.input.focus(); }

  /** 在回答时：框里有字照样发（先排着，蓝图「排队的消息」），空着的发送按钮变成打断；命令照常能执行。 */
  setRunning(running) {
    this.running = running;
    this.el.classList.toggle('is-running', running);
    this.syncButton();
  }

  /** 发送按钮：框里没字、也没有跟着发的，在回答时是打断，不在回答时灰着；跟着发的还在准备（附件在传）也灰着。 */
  syncButton() {
    const parts = this.payload();
    const empty = this.input.value.trim() === '' && !parts.some((p) => p.has());
    const stop = this.running && empty;
    this.sendButton.toggleAttribute('disabled', (empty && !stop) || parts.some((p) => p.busy()));
    if (this.sendButton.classList.contains('is-stop') === stop) return;
    this.sendButton.classList.toggle('is-stop', stop);
    this.sendButton.title = t(stop ? 'interrupt' : 'send');
    replace(this.sendButton, stop ? h('span.stop-mark') : icon('arrow-up'));
  }

  /** 字变了：命令列表照它开关；跟着字长高，最多 `composer_max_rows` 行，再多在框里滚。 */
  changed() {
    this.menu.update(this.input.value);
    const el = this.input;
    // 还没挂进页面时量不出高度（是 0），挂上以后由页面再调一次
    if (!el.isConnected) return;
    const line = parseFloat(getComputedStyle(el).lineHeight) || 24;
    el.style.height = 'auto';
    el.style.height = `${Math.min(el.scrollHeight, line * res.layout.composer_max_rows)}px`;
    this.syncButton();
  }

  /**
   * 回车、点发送：斜杠命令执行它；像命令、没有这个命令的提示「命令不存在」，字留着；别的照普通的话发（蓝图「命令列表」第 4、5 条）。
   * 跟着发的（附件）交出来合进参数；还在准备的不发；核心拒了的放回去（蓝图「附件」第 4 条）。
   */
  async submit() {
    const text = this.input.value;
    const line = read(this.specs(), text);
    if (line.kind === 'command') {
      this.run(line.spec, line.words);
      return;
    }
    if (line.kind === 'unknown') {
      this.say(t('commands.unknown'));
      return;
    }
    const parts = this.payload();
    if (parts.some((p) => p.busy())) {
      this.say(t('payload_busy'));
      return;
    }
    if (text.trim() === '' && !parts.some((p) => p.has())) {
      // 在回答、框里空着：发送按钮是打断
      if (this.running) this.on.interrupt();
      return;
    }
    const taken = parts.map((p) => ({ p, given: p.take() })).filter((x) => x.given);
    this.set('');
    const ok = await this.on.send(text, Object.assign({}, ...taken.map((x) => x.given)));
    if (ok) return;
    // 拒了：字放回去（框里又写了的不覆盖），附件放回去
    if (this.input.value === '') this.set(text);
    for (const x of taken) x.p.putBack(x.given);
  }

  /** 执行一条命令：框清空，命令不进正文、不发给她。 */
  run(spec, words) {
    this.set('');
    this.on.command(spec, words);
  }

  /** 换掉框里的字，光标放在末尾。 */
  set(text) {
    this.input.value = text;
    this.input.setSelectionRange(text.length, text.length);
    this.putBackText = null;
    this.changed();
  }

  /** `Tab` 补全命令：名字填进框里，焦点留着。 */
  fill(text) {
    this.set(text);
    this.input.focus();
  }

  /** 撤销成了：撤掉的那句放回来、整段选中，直接打字就替换掉它；框里已经有字的不动（`tui.md`「输入框」第 7 条）。 */
  putBack(said) {
    if (this.input.value !== '') return;
    this.set(said);
    this.putBackText = said;
    this.input.select();
  }

  /** 恢复了：撤销时放回来的那句还没动过的，收回去，免得回车再发一遍。 */
  takeBack() {
    if (this.putBackText != null && this.input.value === this.putBackText) this.set('');
    this.putBackText = null;
  }

  /**
   * 焦点不在哪个输入框里时按 `/`：只把焦点放回输入框、光标在末尾，不打进这个 `/`（蓝图「按键」，2026-09-30 项目主人定）。
   */
  slash(ev) {
    if (ev.key !== '/' || ev.ctrlKey || ev.metaKey || ev.altKey || ev.isComposing || ev.defaultPrevented) return;
    const at = /** @type {HTMLElement|null} */ (ev.target);
    if (at && (at.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(at.tagName))) return;
    ev.preventDefault();
    this.input.focus();
    this.input.setSelectionRange(this.input.value.length, this.input.value.length);
  }

  key(ev) {
    if (ev.isComposing || ev.keyCode === 229) return;
    if (this.menu.key(ev)) return;
    if (ev.key === 'Enter' && !ev.shiftKey) {
      ev.preventDefault();
      this.submit();
      return;
    }
    // Tab、Shift+Tab：换下一级权限（照 TUI；命令列表开着时 Tab 由列表补全，上面已经接走了）
    if (ev.key === 'Tab' && !ev.ctrlKey && !ev.metaKey && !ev.altKey) {
      ev.preventDefault();
      this.on.cycleLevel();
      return;
    }
    // 两下 Esc：在回答时打断（框里有没有字都算），没在回答、有字时清空
    if (ev.key !== 'Escape' || (!this.running && this.input.value === '')) return;
    ev.preventDefault();
    const now = Date.now();
    if (now - this.esc < res.layout.esc_window_ms) {
      this.esc = 0;
      if (this.running) this.on.interrupt();
      else this.set('');
      return;
    }
    this.esc = now;
    this.say(t(this.running ? 'esc_interrupt_hint' : 'esc_clear_hint'));
  }

  /** 提示：浮在输入框上面的小框，停一会儿；新的顶掉旧的。`good` 的框是绿的（`tui.md`「提示」）。 */
  say(text, good = false) {
    this.notice.textContent = text;
    this.notice.classList.toggle('good', good);
    // 出来从下面升上来，停一会儿淡出（蓝图「动效」）；新的顶掉旧的
    show(this.notice);
    clearTimeout(this.noticeTimer);
    this.noticeTimer = setTimeout(() => hide(this.notice), res.layout.notice_ms);
  }

  /**
   * 框下面那一行：左边级别（级别的颜色，点一下换下一级）、模型（加粗）、端点；右边放得下的几格（`model/footer.js`）。
   * @param {ReturnType<typeof import('../model/footer.js').footer>} f
   */
  drawFooter(f) {
    const { level, label, model, endpoint } = f.left;
    this.drawLevel(level, label);
    const sig = `${model}|${endpoint}`;
    if (this.model.dataset.sig !== sig) {
      this.model.dataset.sig = sig;
      replace(this.model, model ? [h('span.sep', ' · '), h('b', model), h('span.dim', ` ${endpoint}`)] : null);
    }
    this.parts = f.right;
    this.drawRight();
  }

  /**
   * 级别那一格：变了才动。第一次直接写；换了的原地换：旧的先淡出、糊开一点，新的接着淡入、由糊变清（CSS 的 `level-out`、
   * `level-in`），颜色跟着过渡，输入框外面闪一圈新级别颜色的淡光（蓝图「输入框下面那一行」，2026-09-30 项目主人定）。字数不一样
   * 的，那一格的宽度同时从旧的缓到新的（和新字的进场一样长、一样的曲线；退场的旧字不占宽度，CSS 里浮起来），后面的模型名跟着慢慢挪（2026-09-30 项目主人指出：原来动画
   * 放完才一下跳过去）。
   */
  drawLevel(level, label) {
    if (this.drawnLevel === level) return;
    const first = this.drawnLevel == null;
    this.drawnLevel = level;
    this.levelButton.className = `footer-level level-${level}`;
    const next = h('span', label);
    if (first) {
      replace(this.levelRoll, next);
      return;
    }
    // 换之前多宽（上一次还在缓的，量到的是缓到一半的宽）
    const from = widthOf(this.levelRoll);
    this.levelWidth?.cancel();
    // 上一次没走完的（快速连按、系统关了动画时 animationend 不来）先拿掉
    for (const gone of [...this.levelRoll.querySelectorAll('.is-leaving')]) gone.remove();
    for (const old of [...this.levelRoll.children]) {
      old.classList.add('is-leaving');
      old.addEventListener('animationend', () => old.remove(), { once: true });
    }
    next.classList.add('is-entering');
    this.levelRoll.append(next);
    // 宽度跟着新字的进场动画缓过去（系统关了动画的，进场动画没有，宽度直接换）；缓完右边照新的宽重排一次
    const to = widthOf(this.levelRoll);
    const style = getComputedStyle(next);
    const ms = style.animationName === 'none' ? 0 : span(style.animationDuration, style.animationDelay);
    if (from !== to && ms) {
      this.levelWidth = this.levelRoll.animate([{ width: `${from}px` }, { width: `${to}px` }], { duration: ms, easing: style.animationTimingFunction });
      this.levelWidth.finished.then(() => this.drawRight(), () => {});
    }
    this.box.style.setProperty('--flash', `var(--t-${level.replace('_', '-')})`);
    this.box.classList.remove('is-level-flash');
    void this.box.offsetWidth;
    this.box.classList.add('is-level-flash');
  }

  /**
   * 右边：放不下的照先后丢（先速度，再累计，上下文最后），量的是真画出来的宽。左边量本来有多宽
   * （`scrollWidth`，不是被挤窄以后的）：右边先让，左边最后才截掉加 `…`（`tui.md`「框下面那一行」）。
   */
  drawRight() {
    // 中间有按钮的（后台任务），它和两边各隔一个 `footer_gap`，右边先让
    const middle = this.middle.offsetWidth;
    const room = this.footer.clientWidth - this.left.scrollWidth - res.layout.footer_gap - (middle ? middle + res.layout.footer_gap : 0);
    const measure = (parts) => {
      this.right.textContent = parts.map((p) => p.text).join(' · ');
      return this.right.offsetWidth;
    };
    const kept = fit(this.parts, room, measure);
    this.right.textContent = kept.map((p) => p.text).join(' · ');
  }
}

/** 多宽：自己的 CSS 像素、不取整（`offsetWidth` 取整，缓完差不到一个像素也会跳一下）。 */
function widthOf(el) {
  return parseFloat(getComputedStyle(el).width) || 0;
}
