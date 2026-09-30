// @ts-check
//! 输入框这一块（蓝图 `web.md`「输入框」「输入框下面那一行」「提示」「按键」「斜杠命令」）：圆角照 Claude 网页端的输入框，
//! 下面一行照 TUI 的「框下面那一行」，提示照 TUI 的「提示」。
//!
//! 按键照 `tui.md`「按键」：`Enter` 发，`Shift+Enter` 换行，输入法在选字时不算；两下 `Esc` 清空（1.5 秒内）。
//! 框里是斜杠命令的，回车执行它，不发给她（`model/commands.js` 的 `read`）；命令列表开着时 `↑` `↓` `Tab` `Enter` `Esc`
//! 先归列表（`ui/commands.js`）。焦点不在哪个输入框里时按 `/`，只把焦点放回这里，不打进这个 `/`。
//! `↑` `↓` 翻输入历史、`Ctrl+R` 开输入历史列表（蓝图「输入历史」：怎么翻是 `model/history.js` 的 `Recall`，列表在
//! `ui/history.js`）；发出去的话、执行的命令记进去，两下 `Esc` 清掉的那句单独留一份，空着按 `↑` 先拿回它。
//! 光标前面是 `@` 词时开 `@` 选文件的列表（蓝图「`@` 选文件」：词怎么认、写进去的路径在 `model/mention.js`，列表在
//! `ui/mention.js`，列、找由桥做）；选定的图片这些交给跟着话一起发的（附件）收，别的在框里写成一块 `[文件名]`（`model/blocks.js`：
//! 框底下垫一层同样排版的字给块铺底，退格、`Delete` 整块删，发出去换回路径）。
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
import { Picker } from './picker.js';
import { HistoryList } from './history.js';
import { MentionList } from './mention.js';
import { wordAt, plan, pathText, dirWord, splice, failure } from '../model/mention.js';
import { blockLabel, expand, used, erase, pieces } from '../model/blocks.js';
import { Recall } from '../model/history.js';
import { show, hide, span } from '../lib/motion.js';

/**
 * @typedef {{id: string, has: () => boolean, busy: () => boolean, take: () => Record<string, any>|null, putBack: (given: any) => void,
 *   keep?: (given: any) => any, recall?: (saved: {session: string, kept: any}|null) => void, settle?: () => void,
 *   offer?: (refs: any[]) => any[], dropLast?: () => boolean}} Payload
 *   跟着话一起发的一样（挂载位 `composer.payload` 的一件，蓝图 `web/architecture.md`）；`keep`、`recall`、`settle` 是输入历史用的：
 *   交出去的记成能存下来的样子、翻出来的换上（`null` 拿掉）、改了字留下（蓝图「输入历史」第 1、2 条）；`offer` 是 `@` 选文件交过来的
 *   本机文件，收下它认得的，交回不收的（蓝图「`@` 选文件」第 5 条）；`dropLast` 是光标在最前面按退格，拿掉最后一块，拿掉了交回 `true`
 */

export class Composer {
  /**
   * @param {{send: (text: string, extra: Record<string, any>) => Promise<boolean>, interrupt: () => void, cycleLevel: () => void,
   *   command: (spec: import('../model/commands.js').Spec, words: string|null) => void,
   *   history: {load: () => import('../model/history.js').Item[], save: (items: import('../model/history.js').Item[]) => void},
   *   session: () => string|null, files: (params: any) => Promise<any>, where: () => {cwd: string|null, home: string|null}} on
   *   `send` 交回核心收没收；`history` 读、存输入历史（这台设备上、按账号分开）；`session` 正在看的会话；`files` 问桥列、找文件
   *   （`web.files`）；`where` 这个会话的工作目录、家目录（`@` 选文件写路径照它）
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
      spellcheck: 'false',
      placeholder: t('placeholder', { name: res.persona.name }),
      onkeydown: (ev) => this.key(ev),
      oninput: () => this.changed(),
    }));
    // 一开始框里没字：先灰着（原来先是能发的蓝色，挂好了才变灰，刷新时右下角闪一下）
    this.sendButton = h('button.composer-send', { type: 'button', title: t('send'), disabled: true, onclick: () => this.submit() }, icon('arrow-up'));
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
    // 从命令列表里点的、选中回车的：记成 `/名字`
    this.menu = new CommandList({ run: (spec) => { this.remember(`/${spec.name}`); this.run(spec, null); }, fill: (text) => this.fill(text) }, specs);
    /** 翻输入历史（蓝图「输入历史」） */
    this.recall = new Recall(on.history.load());
    this.historyList = new HistoryList({ choose: (item) => this.chosen(item), closed: () => this.input.focus() });
    /** `@` 选文件（蓝图「`@` 选文件」）：现在的词（`key` 是位置加字）、`Esc` 关掉的那个词、问到第几次（旧的回来了不要） */
    this.mention = new MentionList({ pick: (entry, how) => this.pickFile(entry, how), dismiss: () => this.dismissMention() });
    this.mentionAt = /** @type {{start: number, end: number, word: string, key: string}|null} */ (null);
    this.mentionDismissed = /** @type {string|null} */ (null);
    this.mentionSeq = 0;
    this.mentionTimer = 0;
    /** 框里的文件块：名字 → 写进话里的路径（发出去换回去；发成了清掉） */
    this.blocks = /** @type {Map<string, string>} */ (new Map());
    /** 框底下垫的那层字：和框同样排版，只给块铺底（框里的字是框自己画的） */
    this.backdrop = h('div.composer-backdrop', { 'aria-hidden': 'true' });
    this.input.addEventListener('scroll', () => { this.backdrop.scrollTop = this.input.scrollTop; });
    /** 选一样的浮层（`/language`）：和命令列表同一个位置 */
    this.picker = new Picker();
    this.head = h('div.composer-head');
    this.tools = h('span.composer-tools');
    this.float = h('div.composer-float');
    /** 占着整个框的（挂载位 `composer.takeover`：确认和提问的抽屉）；有东西占着时框里原来的让出来（`takeover`） */
    this.takeoverEl = h('div.composer-takeover');
    this.box = h('div.composer', this.notice, this.takeoverEl, this.head, h('div.composer-field', this.backdrop, this.input), this.bar = h('div.composer-bar', this.tools, this.sendButton), this.menu.el, this.picker.el, this.historyList.el, this.mention.el, this.float);
    this.el = h('div.composer-dock', this.box, this.footer);
    this.parts = /** @type {{key: string, text: string}[]} */ ([]);
    /** 撤销时放回框里的那句：恢复时还没动过的收回去（`tui.md`「输入框」第 7 条）。 */
    this.putBackText = /** @type {string|null} */ (null);
    // 那一行变宽变窄、中间的按钮出现消失：右边重新挑放得下的
    const fit = new ResizeObserver(() => this.drawRight());
    fit.observe(this.footer);
    fit.observe(this.middle);
    document.addEventListener('keydown', (ev) => this.slash(ev));
    // 光标挪了（点、左右键）：照光标前面的词重看要不要开 `@` 选文件
    document.addEventListener('selectionchange', () => { if (document.activeElement === this.input) this.syncMention(); });
  }

  focus() { this.input.focus(); }

  /**
   * 挂载位 `composer.takeover` 占不占着框（蓝图「确认和提问」第 2 条）：占着时框里原来的（附件那一排、写字的地方、下面一排）
   * 让出来，字和附件留着；框的高度缓过去（`--takeover-ms`）；收回时焦点回到写字的地方。
   * @param {boolean} open
   */
  takeover(open) {
    if (this.taken === open) return;
    this.taken = open;
    const box = this.box;
    // 写字的地方正在变高变矮（从命令打开的：框里的字刚清掉，正往回缩）：从变之前的高度长过去，不先缩一下再长
    const style = getComputedStyle(this.input);
    const moving = this.settledBox && performance.now() - this.settledBox.at < span(style.transitionDuration, style.transitionDelay);
    const from = open && moving ? this.settledBox.height : box.offsetHeight;
    box.classList.toggle('is-taken', open);
    const to = box.offsetHeight;
    const ms = parseFloat(getComputedStyle(box).getPropertyValue('--takeover-ms')) || 0;
    if (from && to && from !== to && ms && !matchMedia('(prefers-reduced-motion: reduce)').matches) {
      // 缓的时候里面的东西贴着框的下沿（`is-morphing`）：上沿往上长把抽屉从上面露出来，收的时候上沿往下收
      box.classList.add('is-morphing');
      box.animate([{ height: `${from}px` }, { height: `${to}px` }], { duration: ms, easing: 'cubic-bezier(0.2, 0, 0, 1)' })
        .finished.catch(() => {}).finally(() => box.classList.remove('is-morphing'));
    }
    if (!open) {
      this.changed();
      this.input.focus();
    }
  }

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

  /** 字变了：命令列表照它开关；跟着字长高，最多 `composer_max_rows` 行，再多在框里滚。翻出来的字改了：不在翻了，跟着回来的附件留下。 */
  changed() {
    if (this.recall.shown != null && this.input.value !== this.recall.shown) {
      this.recall.reset();
      for (const p of this.payload()) p.settle?.();
    }
    this.menu.update(this.input.value);
    const el = this.input;
    // 还没挂进页面时量不出高度（是 0），挂上以后由页面再调一次
    if (!el.isConnected) return;
    const line = parseFloat(getComputedStyle(el).lineHeight) || 24;
    const max = line * res.layout.composer_max_rows;
    // 变高变矮缓过去（蓝图「动效」，CSS 的 height 过渡）：量新高度要先放开成 auto，量完放回原来的高度、让浏览器记住起点，再定
    // 新的高度；字多过最多那几行的才在框里滚，平时不出滚动条（变高的那一下不闪一根）
    const from = el.offsetHeight;
    // 记下变之前框有多高：紧接着被抽屉占了的，从这个高度长过去，不先缩一下（`takeover`）
    this.settledBox = { height: this.box.offsetHeight, at: performance.now() };
    el.style.height = 'auto';
    const to = Math.min(el.scrollHeight, max);
    el.style.overflowY = el.scrollHeight > max ? 'auto' : 'hidden';
    if (from && from !== to) {
      el.style.height = `${from}px`;
      void el.offsetHeight;
    }
    el.style.height = `${to}px`;
    this.drawBlocks();
    this.syncButton();
    this.syncMention();
  }

  /**
   * 回车、点发送：斜杠命令执行它；像命令、没有这个命令的提示「命令不存在」，字留着；别的照普通的话发（蓝图「命令列表」第 4、5 条）。
   * 跟着发的（附件）交出来合进参数；还在准备的不发；核心拒了的放回去（蓝图「附件」第 4 条）。
   */
  async submit() {
    const text = this.input.value;
    const line = read(this.specs(), text);
    if (line.kind === 'command') {
      this.remember(text);
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
    // 文件块换回路径写在原来的位置（蓝图「`@` 选文件」第 5 条）
    const blocks = used(text, this.blocks);
    const sent = expand(text, this.blocks);
    const ok = await this.on.send(sent, Object.assign({}, ...taken.map((x) => x.given)));
    // 记进输入历史：框里的样子加上用到的块（翻出来照样是块）；发成了的连带过去的附件（核心存好的那一份、发在哪个会话；
    // 新会话发了才有编号）
    const kept = ok ? taken.map((x) => [x.p.id, x.p.keep?.(x.given)]).filter(([, v]) => v) : [];
    const session = this.on.session();
    const extra = {
      ...(kept.length && session ? { session, parts: Object.fromEntries(kept) } : {}),
      ...(blocks.length ? { blocks, sent } : {}),
    };
    this.remember(text, Object.keys(extra).length ? extra : null);
    if (ok) {
      this.blocks = new Map();
      return;
    }
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
    // 在输入历史里找得到的（发出去的样子一样），照框里的样子放回来：块还是块
    const hit = this.recall.items.find((x) => x.sent === said);
    for (const [label, path] of hit?.blocks ?? []) this.blocks.set(label, path);
    const text = hit ? hit.text : said;
    this.set(text);
    this.putBackText = text;
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
    if (ev.key !== '/' || ev.ctrlKey || ev.metaKey || ev.altKey || ev.isComposing || ev.defaultPrevented || this.taken) return;
    const at = /** @type {HTMLElement|null} */ (ev.target);
    if (at && (at.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(at.tagName))) return;
    ev.preventDefault();
    this.input.focus();
    this.input.setSelectionRange(this.input.value.length, this.input.value.length);
  }

  key(ev) {
    if (ev.isComposing || ev.keyCode === 229) return;
    if (this.mention.key(ev)) return;
    if (this.menu.key(ev)) return;
    // ↑ ↓：翻输入历史；不接的归浏览器挪光标
    if ((ev.key === 'ArrowUp' || ev.key === 'ArrowDown') && !ev.shiftKey && !ev.ctrlKey && !ev.metaKey && !ev.altKey) {
      const up = ev.key === 'ArrowUp';
      const text = up ? this.recall.older(this.input.value, this.caret()) : this.recall.newer(this.input.value, this.caret());
      if (text == null) return;
      ev.preventDefault();
      this.put(text);
      this.bring(this.recall.item());
      return;
    }
    // 块整块删：光标在块后面退格、在块前面 Delete（蓝图「`@` 选文件」第 5 条）
    if ((ev.key === 'Backspace' || ev.key === 'Delete') && !ev.ctrlKey && !ev.metaKey && !ev.altKey && this.input.selectionStart === this.input.selectionEnd) {
      const labels = [...this.blocks.keys()].filter((l) => this.input.value.includes(l));
      const cut = erase(this.input.value, this.input.selectionStart, labels, ev.key === 'Backspace' ? 'back' : 'forward');
      if (cut) {
        ev.preventDefault();
        this.input.value = cut.value;
        this.input.setSelectionRange(cut.caret, cut.caret);
        this.changed();
        return;
      }
    }
    // 光标在最前面（没选着字）按退格：拿掉最后一张附件（卡排在字的前面，照 TUI 退格整块删块；蓝图「附件」第 3 条）
    if (ev.key === 'Backspace' && !ev.ctrlKey && !ev.metaKey && !ev.altKey && this.input.selectionStart === 0 && this.input.selectionEnd === 0) {
      if (this.payload().some((p) => p.dropLast?.())) ev.preventDefault();
      return;
    }
    // Ctrl+R：输入历史列表（只在这个框里接，别处照浏览器的刷新）
    if (ev.key.toLowerCase() === 'r' && ev.ctrlKey && !ev.shiftKey && !ev.altKey && !ev.metaKey) {
      ev.preventDefault();
      this.openHistory();
      return;
    }
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
      else {
        // 清掉的那句单独留一份：空着按 ↑ 先拿回它
        this.recall.clear(this.input.value);
        this.set('');
        this.say(t('cleared'));
      }
      return;
    }
    this.esc = now;
    this.say(t(this.running ? 'esc_interrupt_hint' : 'esc_clear_hint'));
  }

  /** 光标在哪：在最前面（没选着字）、前面没有换行、后面没有换行（`Recall` 照它定翻不翻）。 */
  caret() {
    const { value, selectionStart: a, selectionEnd: b } = this.input;
    const none = a === b;
    return { start: none && a === 0, first: none && !value.slice(0, a).includes('\n'), last: none && !value.slice(b).includes('\n') };
  }

  /** 翻出来的放进框里，光标在末尾；是命令的不弹命令列表（字改了照常弹）。 */
  put(text) {
    this.menu.menu.dismiss(text);
    this.set(text);
  }

  /**
   * 翻到的那一条带的附件回到框里（换掉上一次跟着翻出来的）；`null`（没发的那句、拿回的清掉的那句）只拿掉跟着翻出来的。
   * @param {import('../model/history.js').Item|null} item
   */
  bring(item) {
    for (const [label, path] of item?.blocks ?? []) this.blocks.set(label, path);
    this.drawBlocks();
    for (const p of this.payload()) p.recall?.(item?.session && item.parts?.[p.id] ? { session: item.session, kept: item.parts[p.id] } : null);
  }

  /** 发出去了一句（话或命令）：记进输入历史（带过去的附件一起），存下来。 */
  remember(text, extra = null) {
    this.recall.record(text, Date.now(), res.layout.history_max, extra);
    this.on.history.save(this.recall.items);
  }

  /** `Ctrl+R`：开输入历史列表（和命令列表、选语言的浮层不同时开）；还没发过话的只提示一句。 */
  openHistory() {
    if (!this.recall.items.length) {
      this.say(t('history.empty'));
      return;
    }
    this.picker.close();
    this.menu.menu.dismiss(this.input.value);
    this.menu.update(this.input.value);
    this.historyList.show(this.recall.items);
  }

  /** 列表里选定了一条：放进框里，带的附件回到框里、留下；框里原来有字的记进输入历史，不丢。 */
  chosen(item) {
    const had = this.input.value;
    if (had.trim() && had !== item.text) this.remember(had);
    this.recall.reset();
    this.put(item.text);
    this.bring(item);
    for (const p of this.payload()) p.settle?.();
    this.input.focus();
  }

  /**
   * `@` 选文件：光标前面是 `@` 词（没选着字）的，隔一小会儿（`mention_delay_ms`）问桥列、找，旧的回来了不要；开列表时带
   * `fresh`（清单隔一阵的重建）。不是的、命令列表或输入历史列表开着的关掉；`Esc` 关掉的那个词没变就一直关着。
   */
  syncMention() {
    const el = this.input;
    const w = el.selectionStart === el.selectionEnd ? wordAt(el.value, el.selectionStart) : null;
    if (!w || this.menu.open || this.historyList.open) {
      if (!w) this.mentionDismissed = null;
      this.mentionAt = null;
      this.mentionSeq += 1;
      this.mention.close();
      return;
    }
    const key = `${w.start}:${w.word}`;
    if (key === this.mentionDismissed || key === this.mentionAt?.key) return;
    const fresh = !this.mention.open;
    this.mentionAt = { ...w, key };
    const seq = ++this.mentionSeq;
    clearTimeout(this.mentionTimer);
    this.mentionTimer = window.setTimeout(async () => {
      // 问不到的写清楚为什么（桥太旧、工作目录不在、数据目录不列……），不画成空列表
      const found = await this.on.files({ ...plan(w.word), ...(fresh ? { fresh: true } : {}) })
        .catch((err) => ({ items: [], partial: false, layer: false, error: failure(err) }));
      if (seq === this.mentionSeq) this.mention.show({ word: w.word, ...found });
    }, res.layout.mention_delay_ms);
  }

  /** `Esc`：关掉，这个词没变就不再开。 */
  dismissMention() {
    this.mentionDismissed = this.mentionAt?.key ?? null;
    this.mentionSeq += 1;
    this.mention.close();
  }

  /**
   * 选定了一条（蓝图「`@` 选文件」第 5 条）：`Tab` 在目录上是进这个目录接着列；图片、PDF、音频、视频交给附件收（词拿掉）；
   * 别的文件、目录把词换成它的路径（后面补一个空格）。
   */
  pickFile(entry, how) {
    const at = this.mentionAt;
    if (!at) return;
    const p = plan(at.word);
    const put = (text) => {
      const next = splice(this.input.value, at.start, at.end, text);
      this.input.value = next.value;
      this.input.setSelectionRange(next.caret, next.caret);
      this.changed();
    };
    if (entry.dir && how === 'tab') {
      put(dirWord(p.mode === 'dir' ? p.dir + entry.path : entry.path));
      return;
    }
    this.mentionSeq += 1;
    this.mention.close();
    this.mentionAt = null;
    if (!entry.dir) {
      const name = entry.full.split('/').pop() ?? entry.full;
      let left = [{ name, size: entry.size ?? 0, type: entry.type ?? '', path: entry.full }];
      for (const part of this.payload()) if (part.offer) left = part.offer(left);
      if (!left.length) {
        put('');
        return;
      }
    }
    // 别的文件、目录：框里写成一块 `[文件名]`，记着它的路径，发出去换回去
    const where = this.on.where();
    const path = pathText(entry.full, entry.dir, where.cwd, where.home);
    const label = blockLabel(path, entry.dir, this.blocks, res.layout.block_name_max);
    this.blocks.set(label, path);
    put(`${label} `);
  }

  /** 框底下垫的那层字：和框里的字一样，块铺底（结尾补一个空格：框里最后是换行时，垫的那层也要多一行）。 */
  drawBlocks() {
    const value = this.input.value;
    const labels = [...this.blocks.keys()].filter((l) => value.includes(l));
    const sig = `${value}\u0000${labels.join('\u0000')}`;
    if (this.backdrop.dataset.sig === sig) return;
    this.backdrop.dataset.sig = sig;
    replace(this.backdrop, [...pieces(value, labels).map((p) => (p.block ? h('mark.composer-block', p.text) : p.text)), ' ']);
    this.backdrop.scrollTop = this.input.scrollTop;
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
