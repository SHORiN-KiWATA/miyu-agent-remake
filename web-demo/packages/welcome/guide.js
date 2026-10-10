// @ts-check
//! 第一次引导的整页（软件包 `welcome`，蓝图 `web.md`「第一次引导」第 2、10、11 条）：盖着整页的一层，一屏一屏地走：欢迎 → 界面语言 →
//! 模型（`settings` 包的 `providers`）→ 人格、预设（`setup` 包的 `personas`）→ 完成。每一屏只交标题、内容和主按钮，这里画顶上一行
//! （步骤条）、上一步、下一步，管 `Enter`、`↑` `↓`、换屏的动画和吉祥物站哪。

import { h, icon, replace } from '../../src/lib/dom.js';
import { helloScreen, languageScreen, doneScreen, STEP_KEY, DONE_KEY } from './screens.js';

/** 步子的先后；`back` 是上一步回到哪 */
const BACK = /** @type {Record<string, string|null>} */ ({ hello: null, language: 'hello', model: 'language', persona: 'model', preset: 'persona', done: 'preset' });
/** 进步骤条的几步 */
export const NUMBERED = ['language', 'model', 'persona', 'preset'];
/** 换一屏要选中的选项（`↑` `↓` 换着选的：选了就是它；别的只挪焦点） */
const CHOICES = '.ob-row:not([disabled]), .wl-lang, .setup-choice, .ob-model';
const SELECTS = new Set(['wl-lang', 'setup-choice', 'ob-model']);
/** 吉祥物站的那一块：这一屏内容里第一块看得见的（第一行选项、第一个框），站在它上沿靠右（2026-10-10 项目主人选的，同对话页站在输入框上） */
const SURFACES = '.ob-row, .wl-lang, .setup-choice, .ob-input, .set-input, .setup-area, .set-select';
/** 平常站的地方离那一块右边多远 */
const HOME_RIGHT = 40;

/**
 * @typedef {{title: string, sub?: string|null, body: HTMLElement, back?: (() => void)|null, center?: boolean, kind?: string,
 *   next?: {label?: string|(() => string), ready: () => boolean, run: () => Promise<string|null>}|null, focus?: HTMLElement|null}} Screen
 * @typedef {{model: {id: string, model: string|null, name: string}|null, persona: string, preset: string}} Result 走完写的
 */

export class Guide {
  /**
   * @param {any} ctx `welcome` 包的 ctx @param {() => void} uncover 揭掉一打开就盖上的那层底色（收起时）
   * @param {() => void} release 放开藏着的吉祥物（给了它舞台以后）
   */
  constructor(ctx, uncover, release) {
    this.ctx = ctx;
    this.uncover = uncover;
    this.release = release;
    this.t = (/** @type {string} */ key, /** @type {any} */ fields) => ctx.text(key, fields);
    this.step = 'hello';
    /** @type {Screen|null} */
    this.screen = null;
    /** 画着的那一屏是哪一步的（等待屏之后接着画的照它认） @type {string|null} */
    this.screenStep = null;
    this.running = false;
    /** @type {Result} */
    this.result = { model: null, persona: '', preset: '' };
    /** 存好的人格、预设的编号：走完开的新会话照它选 */
    this.ids = /** @type {{persona?: string|null, preset?: string|null}} */ ({});
    this.slot = h('div.wl-slot');
    this.steps = h('ol.wl-steps');
    this.head = h('div.wl-head', h('span.wl-brand', this.t('brand')), this.steps);
    this.page = h('div.wl-page', h('div.wl-col', this.head, this.slot));
    this.layer = h('div.wl-layer', { role: 'dialog', 'aria-modal': 'true' }, sky(ctx.config.stars), this.page);
    /** @type {HTMLButtonElement|null} */
    this.nextBtn = null;
    this.error = h('p.wl-error', { hidden: true });
    /** 吉祥物站的舞台：欢迎页一种、别的几屏一种；撤销的函数 */
    this.stageKind = '';
    this.unstage = /** @type {(() => void)|null} */ (null);
    /** 画了第几屏：台子的编号带着它（换了一屏，旧的台子就没了，吉祥物照新的站） */
    this.shown = 0;
    /** 人格、预设还在读的那一下：台子照上一屏的留着，不让她掉下去 @type {{platforms: any[], home: any}|null} */
    this.held = null;
    this.onKey = (/** @type {KeyboardEvent} */ e) => this.key(e);
    this.onFocus = (/** @type {FocusEvent} */ e) => {
      // 焦点关在引导里：后面的页面点不到、Tab 不过去
      if (!this.layer.contains(/** @type {Node} */ (e.target))) this.focusScreen();
    };
    this.onScroll = () => this.ctx.mascot?.refresh?.();
  }

  /** 和设置页一个样子的控件（`settings` 包给的；它比这个包晚加载，用到时再要） */
  get kit() {
    this.madeKit ??= this.ctx.settings.kit(this.layer, (/** @type {string} */ text) => this.say(text));
    return this.madeKit;
  }

  /** 盖上，从 `step` 开始。 @param {string} step */
  open(step) {
    document.body.append(this.layer);
    this.layer.addEventListener('keydown', this.onKey);
    document.addEventListener('focusin', this.onFocus);
    this.page.addEventListener('scroll', this.onScroll, { passive: true });
    this.go(step in BACK ? step : 'hello', 1);
    // 吉祥物的包比这个包晚加载：来了再让它上舞台
    const wait = (/** @type {number} */ tries) => {
      if (this.stageKind || !this.layer.isConnected) return;
      if (this.ctx.mascot) this.restage();
      else if (tries > 0) setTimeout(() => wait(tries - 1), 100);
      else this.release();
    };
    wait(50);
  }

  /** 收起：淡出，吉祥物跳回输入框上。 */
  close() {
    this.layer.removeEventListener('keydown', this.onKey);
    document.removeEventListener('focusin', this.onFocus);
    this.page.removeEventListener('scroll', this.onScroll);
    this.unstage?.();
    this.unstage = null;
    this.layer.classList.add('is-leaving');
    this.uncover();
    const gone = () => this.layer.remove();
    if (matchMedia('(prefers-reduced-motion: reduce)').matches) gone();
    else this.layer.addEventListener('animationend', gone, { once: true });
  }

  /** 走到一步。 @param {string} step @param {1|-1} dir */
  async go(step, dir) {
    this.step = step;
    this.ctx.mascot?.busy?.(false, false);
    const host = { ready: () => this.sync(), mascot: this.ctx.mascot ?? null, panel: this.layer };
    if (step === 'hello') this.show(helloScreen(this), dir);
    else if (step === 'language') this.show(languageScreen(this), dir);
    else if (step === 'model') {
      const flow = this.ctx.settings.providers({
        ...host,
        show: (/** @type {Screen} */ screen, /** @type {1|-1} */ d) => { if (this.step === 'model') this.show(screen, d); },
        done: (/** @type {Result['model']} */ got) => {
          this.result.model = got;
          this.go('persona', 1);
        },
      });
      flow.start();
    } else if (step === 'persona' || step === 'preset') {
      const part = step === 'persona' ? this.ctx.personas.persona(this.kit, host) : this.ctx.personas.preset(this.kit, host);
      this.show({ title: '', body: h('p.wl-wait', icon('loader-circle')), next: null, kind: 'wait' }, dir);
      await part.load();
      if (this.step !== step) return;
      const screen = part.screen();
      this.show({
        ...screen,
        next: {
          ready: screen.next.ready,
          run: async () => {
            const why = await screen.next.run();
            if (why) return why;
            this.result[step] = part.label();
            this.ids[step] = part.id;
            this.go(step === 'persona' ? 'preset' : 'done', 1);
            return null;
          },
        },
      }, dir);
    } else if (step === 'done') this.show(doneScreen(this), dir);
  }

  /**
   * 画一屏。读完接着等待屏画的（同一步）沿用那个节点、只换里面的：滑进来的动画接着走，不再从头淡入一遍（2026-10-10 项目主人：点下一步
   * 文字那一块闪一下，原来等待屏、读到的那一屏各淡入一遍）。
   * @param {Screen} screen @param {1|-1} dir 往前走的从右边滑进来，往回的从左边
   */
  show(screen, dir) {
    const after = this.screen?.kind === 'wait' && this.screenStep === this.step;
    this.screen = screen;
    this.screenStep = this.step;
    this.running = false;
    const stepBack = BACK[this.step];
    const back = screen.back ?? (stepBack && this.step !== 'done' ? () => this.go(stepBack, -1) : null);
    const next = screen.next ?? null;
    this.nextBtn = next ? /** @type {HTMLButtonElement} */ (h('button.wl-btn.is-primary', { type: 'button', onclick: () => this.run() }, this.labelOf(next))) : null;
    this.error = h('p.wl-error', { hidden: true });
    const foot = back || this.nextBtn
      ? h('div.wl-foot', back ? h('button.wl-link', { type: 'button', onclick: back }, icon('chevron-left'), h('span', this.t('back'))) : h('span'), this.nextBtn)
      : null;
    const parts = [screen.title ? h('h1.wl-title', screen.title) : null, screen.sub ? h('p.wl-sub', screen.sub) : null, h('div.wl-body', screen.body), this.error, foot];
    const old = /** @type {HTMLElement|null} */ (after ? this.slot.querySelector(':scope > .wl-screen') : null);
    const node = old ?? h(`div.wl-screen.is-${dir > 0 ? 'fwd' : 'back'}`);
    node.classList.toggle('is-center', !!screen.center);
    replace(node, ...parts.filter(Boolean));
    this.layer.classList.toggle('is-hello', this.step === 'hello');
    this.layer.classList.toggle('is-center', !!screen.center);
    this.head.hidden = !NUMBERED.includes(this.step);
    this.drawSteps();
    // 还在读的那一下：吉祥物照上一屏的台子站着（先记下来再换掉）
    const waiting = screen.kind === 'wait';
    if (waiting && !this.held && this.stageKind) this.held = { platforms: this.platforms(), home: this.home() };
    if (!waiting) this.held = null;
    if (node !== old) replace(this.slot, node);
    if (!waiting) this.shown += 1;
    this.page.scrollTop = 0;
    this.sync();
    queueMicrotask(() => this.focusScreen());
    if (!this.restage() && !waiting) this.ctx.mascot?.home?.();
  }

  /** 焦点放到这一屏要的地方（没有的放到主按钮）。 */
  focusScreen() {
    const target = this.screen?.focus?.isConnected ? this.screen.focus : this.nextBtn ?? this.slot.querySelector('button');
    target?.focus({ preventScroll: true });
  }

  /** 步骤条：现在这一步加粗，中间短线。 */
  drawSteps() {
    const at = NUMBERED.indexOf(this.step);
    replace(this.steps, NUMBERED.map((s, i) => h(`li${i === at ? '.is-now' : ''}`, `${i + 1} ${this.t(`steps.${s}`)}`)));
  }

  /** 主按钮能不能点、写什么（测试连接那一屏的跟着测的结果变）。 */
  sync() {
    const next = this.screen?.next;
    if (!this.nextBtn || !next) return;
    this.nextBtn.disabled = this.running || !next.ready();
    const label = this.labelOf(next);
    if (this.nextBtn.textContent !== label) this.nextBtn.textContent = label;
  }

  /** 主按钮上的字：没写的是「下一步」。 @param {NonNullable<Screen['next']>} next */
  labelOf(next) {
    return (typeof next.label === 'function' ? next.label() : next.label) ?? this.t('next');
  }

  /** 点主按钮：跑这一屏的 `run`，出错的写在按钮上面。 */
  async run() {
    const screen = this.screen;
    const next = screen?.next;
    if (!next || this.running || !next.ready()) return;
    this.running = true;
    this.error.hidden = true;
    this.sync();
    let why = null;
    try {
      why = await next.run();
    } catch (err) {
      why = /** @type {any} */ (err)?.message ?? String(err);
    }
    if (this.screen !== screen) return;
    this.running = false;
    if (why) this.say(why);
    this.sync();
  }

  /** 一句出错的字（主按钮上面，红字）。 @param {string} text */
  say(text) {
    this.error.hidden = !text;
    this.error.textContent = text;
  }

  /** 按键：`Enter` 是主按钮（多行框里照常换行，焦点在别的按钮上的照按钮自己的），`↑` `↓` 换选项，`Esc` 不关。 @param {KeyboardEvent} e */
  key(e) {
    const target = /** @type {HTMLElement} */ (e.target);
    const typing = target instanceof HTMLTextAreaElement || (target instanceof HTMLInputElement && target.type !== 'button');
    e.stopPropagation();
    // `Esc` 不关引导：开着的下拉菜单收起来（同设置页的 `set-dismiss`）
    if (e.key === 'Escape') {
      this.layer.querySelector('.set-menu')?.dispatchEvent(new CustomEvent('set-dismiss'));
      return;
    }
    if (e.key === 'Enter' && !e.shiftKey && !e.isComposing) {
      // 选项（语言、预设、模型）选中了就是它：回车照主按钮；别的按钮（一家供应商、上一步）照按钮自己的
      const choice = target instanceof HTMLButtonElement && [...SELECTS].some((c) => target.classList.contains(c));
      if (target instanceof HTMLTextAreaElement || (target instanceof HTMLButtonElement && target !== this.nextBtn && !choice)) return;
      e.preventDefault();
      this.run();
    } else if ((e.key === 'ArrowDown' || e.key === 'ArrowUp') && !typing) {
      const all = /** @type {HTMLElement[]} */ ([...this.slot.querySelectorAll(CHOICES)]);
      if (!all.length) return;
      e.preventDefault();
      const at = all.indexOf(target) >= 0 ? all.indexOf(target) : all.findIndex((x) => x.classList.contains('is-on'));
      const to = all[Math.max(0, Math.min(all.length - 1, at + (e.key === 'ArrowDown' ? 1 : -1)))];
      if ([...SELECTS].some((c) => to.classList.contains(c))) {
        const cls = [...SELECTS].find((c) => to.classList.contains(c));
        const index = [...this.slot.querySelectorAll(`.${cls}`)].indexOf(to);
        to.click();
        // 选了以后那一列会重画：焦点放到新画的同一个
        /** @type {HTMLElement|undefined} */ ([...this.slot.querySelectorAll(`.${cls}`)][index])?.focus({ preventScroll: false });
      } else to.focus();
    }
  }

  /** 吉祥物站哪（第 11 条）：欢迎页放大、站在标题上面，别的几屏站在内容一列的上沿。交回换没换舞台（没换的由 `show` 叫它回家）。 */
  restage() {
    const mascot = this.ctx.mascot;
    if (!mascot?.stage) return true;
    const kind = this.step === 'hello' ? 'hello' : 'page';
    if (kind === this.stageKind) return false;
    const first = !this.stageKind;
    this.stageKind = kind;
    // 新的舞台直接换掉旧的（不先撤：撤了它先往输入框跳，新舞台就跳不成了）
    this.unstage = mascot.stage({
      root: this.layer,
      scale: kind === 'hello' ? this.ctx.config.mascot_scale : 1,
      drop: first,
      platforms: () => this.platforms(),
      home: () => this.home(),
    });
    this.release();
    return true;
  }

  /** 台子：站的那一块（居中的几屏是标题正上方一段）；别的选项、按钮那一排的上沿（拖下来松手落到它们上面）。 */
  platforms() {
    if (this.held) return this.held.platforms;
    const top = this.top();
    const list = top ? [top] : [];
    if (this.stageKind === 'page') {
      const first = this.first();
      this.slot.querySelectorAll('.ob-row, .wl-lang, .setup-choice, .wl-foot').forEach((el, i) => {
        if (el === first) return;
        const r = el.getBoundingClientRect();
        if (r.width && r.top > 0) list.push({ id: `wl-${this.shown}-${i}`, x1: r.left + 8, x2: r.right - 8, y: r.top });
      });
    }
    return list;
  }

  /** 这一屏内容里第一块看得见的。 */
  first() {
    return /** @type {HTMLElement|null} */ (this.slot.querySelector('.wl-body')?.querySelector(SURFACES) ?? null);
  }

  /** 站的那一块：居中的那几屏（欢迎、完成）是标题正上方一段，别的是内容里第一块看得见的上沿。 */
  top() {
    const id = `wl-top-${this.shown}`;
    if (this.screen?.center) {
      const title = /** @type {HTMLElement|null} */ (this.slot.querySelector('.wl-title'));
      const r = title?.getBoundingClientRect();
      if (!r?.width) return null;
      const c = r.left + r.width / 2;
      return { id, x1: c - 80, x2: c + 80, y: r.top - 12 };
    }
    const r = this.first()?.getBoundingClientRect();
    if (!r?.width) return null;
    return { id, x1: r.left + 8, x2: r.right - 8, y: r.top };
  }

  /** 平常站哪：居中的几屏在标题正上方；别的在第一块的上沿靠右。 */
  home() {
    if (this.held) return this.held.home;
    const platform = this.top();
    if (!platform) return null;
    if (this.screen?.center) return { x: (platform.x1 + platform.x2) / 2, platform };
    return { x: platform.x2 + 8 - HOME_RIGHT, platform };
  }

  /** 走完（完成那一屏的「开始聊天」）：写 `ui.welcomed`，收起，进一个新的空会话。 */
  async finish() {
    await this.ctx.core.request('config.set', { layer: 'personal', changes: [{ key: 'ui.welcomed', value: true }] });
    this.ctx.storage.set(STEP_KEY, null);
    this.ctx.storage.set(DONE_KEY, true);
    this.close();
    this.ctx.chat.open(null);
    // 新会话照刚设的默认人格、预设（不照上一次的会话）
    const pick = Object.fromEntries(Object.entries(this.ids).filter(([, v]) => v));
    if (Object.keys(pick).length) this.ctx.chat.setDraft(pick);
    this.ctx.composer.focus();
    return null;
  }
}

/** 欢迎页后面那一层：两团慢慢飘的光、一些慢慢闪的星点（随机散着）。 @param {number} stars */
function sky(stars) {
  const dots = Array.from({ length: stars }, () => h('i.wl-star', {
    style: `left: ${(Math.random() * 100).toFixed(1)}%; top: ${(Math.random() * 100).toFixed(1)}%; animation-delay: ${(-Math.random() * 6).toFixed(2)}s; --size: ${Math.random() < 0.3 ? 2 : 1}px`,
  }));
  return h('div.wl-sky', { 'aria-hidden': 'true' }, h('i.wl-glow.is-a'), h('i.wl-glow.is-b'), dots);
}
