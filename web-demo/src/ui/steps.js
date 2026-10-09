// @ts-check
//! 时间线的一步（蓝图 `web.md`「时间线」，照旧版网页 `app.js:6962-7051`、`8574-8771`、`diff.js`）：思考一行、工具一行，
//! 下面接在想时的窗口、命令本身、点开的细节。
//!
//! 一步是一个一直在的节点，流式的字来了只改里面的字：新来的一步淡入一次，在想的标题的流光不断（重建节点会从头来）。
//! 点开、收起记在这一步里，人点过的照人点的；没点过的照 `timeline.json` 的 `expand`。

import { fromUnified, samePath } from '../model/diff.js';
import { h, icon, replace } from './dom.js';
import { spring } from '../lib/motion.js';
import { res, t } from '../util/res.js';
import { row, peek, messagePeek, thinkingTail, commandLines, details, kindOf } from '../model/words.js';
import { imageCard } from './media.js';
import { blobUrl } from '../core/host.js';

export class StepView {
  /**
   * @param {import('../model/timeline.js').Step} step 第一次的样子：种类（思考、工具）以后不变
   * @param {import('./rich.js').Where} where 看着的会话在哪：家目录（对象里的路径写成 `~/…`）
   * @param {boolean} fresh 新来的：淡入一次（读回来的历史不淡入）
   * @param {() => void} [foldSegment] 收起它在的那一段：铺开画（`bare`）的时候，点内容收的是整段
   */
  constructor(step, where, fresh, foldSegment = () => {}) {
    this.where = where;
    this.foldSegment = foldSegment;
    this.home = where.home;
    /** 人点过：开还是关；没点过的是 `null`。 */
    this.open = /** @type {boolean|null} */ (null);
    this.step = step;
    this.thought = step.kind === 'thought';
    this.iconName = '';
    this.drawnBody = '';
    this.drawnCommand = '';
    this.node = h('span.tl-node');
    this.name = h('span.tl-name');
    this.time = h('span.tl-time');
    /** 一段里只有这一步、人点开了那一段：不画这一行，直接铺开内容（`timeline.js` 的 `bare`）。 */
    this.bare = false;
    // 先告诉对话区钉住被点的这一行（内容往下长，`follow.js`），再开、关
    const toggle = guard(() => {
      // 铺开画的（一段里只有这一步、人点开了那一段）：这一步一直开着，点内容收起整段（2026-09-30 项目主人：原来只能点那一段的
      // 收起那一行才收得起）
      if (this.bare) {
        this.foldSegment();
        return;
      }
      this.row.dispatchEvent(new CustomEvent('tl-toggle', { bubbles: true }));
      this.open = !this.opened();
      this.update(this.step, this.spinning, this.live);
    });
    if (this.thought) {
      this.peek = h('span.tl-peek', h('span'));
      // 那一小段前面的 `·` 单独一个：放不下时那一小段左边淡出，点不跟着淡没
      this.row = h('button.tl-row', { type: 'button', onclick: toggle },
        this.node, this.name, this.time, h('span.tl-peek-dot', t('timeline.peek_sep')), this.peek, h('span.tl-chevron', icon('chevron-right')));
      // 在想时滚着的那几行：和那一行一样能点（2026-09-30 项目主人：原来只能点那一行）
      this.window = h('div.tl-window', { onclick: toggle });
      // 点开的全文也整块能点：点了收起（选着字的时候不收，`guard`），悬停亮一档（2026-09-30 项目主人定）
      this.body = h('div.tl-thought-body', { onclick: toggle });
      // 全文放在和工具细节同一种收放的壳里：点开、收起都有动画
      this.el = h(`div.tl-step.is-thought${fresh ? '.is-new' : ''}`, this.row, this.window, h('div.tl-fold', h('div.tl-fold-inner', this.body)));
    } else {
      this.subject = h('span.tl-subject');
      this.said = h('span.tl-said');
      // 编辑、写入：加减的行数（`+3 -1`）
      this.diff = h('span.tl-diff');
      this.status = h('span.tl-status');
      // 留言：收着时那一行后面接留言开头的预览（2026-10-01 项目主人定）
      const message = kindOf(step.name) === 'message';
      // 编号和预览中间一个 `·`（2026-10-01 项目主人定）
      this.peek = message ? h('span.tl-peek', h('span.tl-peek-sep', t('timeline.peek_sep')), h('span')) : null;
      this.row = h('button.tl-row', { type: 'button', onclick: toggle },
        this.node, h('span.tl-head', this.name, this.time, this.subject, this.said, this.diff), this.peek, this.status);
      this.command = h('div.tl-command', { onclick: toggle });
      // 点开的细节：点这一块也收起（点的是链接、按钮、图，或者拖选了字的不算；蓝图「压缩、清空」里的开关规矩）
      this.body = h('div.tl-body', {
        onclick: (/** @type {MouseEvent} */ e) => {
          if (/** @type {Element} */ (e.target).closest('a, button, input, textarea, .conversation-media')) return;
          toggle(e);
        },
      });
      // 收着时结果里的图（读图）画在预览区，小一点；点开换成细节里的那张（蓝图「图片」第 2 条）
      this.thumbs = h('div.tl-thumbs', { hidden: true });
      this.el = h(`div.tl-step.is-tool${message ? '.is-message' : ''}${fresh ? '.is-new' : ''}`, this.row, this.command, this.thumbs, h('div.tl-fold', h('div.tl-fold-inner', this.body)));
    }
    this.spinning = false;
  }

  /** 现在点开着没有。不画那一行的一直开着。 */
  opened() {
    if (this.bare) return true;
    if (this.open != null) return this.open;
    const expand = res.timeline.expand;
    const kind = this.thought ? 'thought' : kindOf(this.step.name);
    return kind ? !!expand[kind] : false;
  }

  /**
   * 照这一步现在的样子改字。
   * @param {import('../model/timeline.js').Step} step
   * @param {boolean} spinning 这一段正在动的那一步（同一时刻只转一处）
   * @param {boolean} [live] 这一段还在进行：想完的思考照旧露着最后几行，等这一段收起（照 TUI，不一条一条地缩）
   */
  update(step, spinning, live = false) {
    this.live = live;
    this.step = step;
    this.spinning = spinning;
    const r = row(step, this.home);
    const open = this.opened();
    this.el.classList.toggle('is-open', open);
    this.el.classList.toggle('is-bare', this.bare);
    this.el.classList.toggle('is-failed', r.failed);
    this.el.classList.toggle('is-live', step.state !== 'done');
    this.el.classList.toggle('is-preparing', step.state === 'preparing');
    this.setIcon(r.icon, step.state === 'preparing');
    this.name.textContent = r.name;
    setTime(this.time, r, step.state === 'preparing');
    if (step.kind === 'thought') this.updateThought(step, open);
    else this.updateTool(step, r, open, spinning);
  }

  /**
   * 思考：收着时接那一小段；这一段还在进行、收着时下面滚着显示最后几行（想完了也留着，照 TUI「时间线」第 5 条：原来想完
   * 就缩成一行，一条思考缩一次，正文跟着上下跳）；点开是全部的字。
   */
  updateThought(step, open) {
    const tail = /** @type {HTMLElement} */ (this.peek?.firstChild);
    tail.textContent = peek(step);
    // 放不下时从右往左推、左边淡出（照旧版 setReasoningPeek）
    requestAnimationFrame(() => this.peek?.classList.toggle('is-overflow', tail.offsetWidth > (this.peek?.clientWidth ?? 0)));
    const rolling = this.live && !open && !!step.text.trim();
    this.window.hidden = !rolling;
    this.el.classList.toggle('is-rolling', rolling);
    if (rolling) this.roll(step.text.trim());
    else this.rollText = '';
    // 收起时字留着，收的动画里还看得到；点开时照最新的字接上
    if (open) this.thinkText(step.text.trim());
  }

  /**
   * 在想时滚着的那几行（蓝图「时间线」思考的预览）：一行一块，只往后接，前面的只留两屏。来了新字，每一帧自己往底下追一截
   * （临界阻尼的弹簧 `spring`，时间常数 `roll_ms`），起步不猛、字一阵阵来也是连着滑，不一下跳上去（2026-10-01、10-02 项目主人指出：原来每来一段重开一次
   * 浏览器的平滑滚动，Firefox 里接不上）。滚出去的行删掉时自己把位置补回来（CSS 关了滚动锚定）。刚露出来的那一下直接到底。
   * @param {string} text
   */
  roll(text) {
    const box = this.window;
    const keep = res.timeline.thinking_rows * 2;
    if (!this.rollText || !text.startsWith(this.rollText)) {
      // 刚露出来、字不是接着原来的：只排最后两屏，直接到底
      this.rollText = text;
      box.replaceChildren(...thinkingTail(text, keep).split('\n').map((line) => h('div.tl-think-line', line)));
      box.scrollTop = box.scrollHeight;
      this.rollAt = { pos: box.scrollTop, vel: 0 };
      return;
    }
    const added = text.slice(this.rollText.length);
    if (!added) return;
    this.rollText = text;
    const parts = added.split('\n');
    const tail = /** @type {HTMLElement} */ (box.lastElementChild);
    if (tail.firstChild) /** @type {Text} */ (tail.firstChild).appendData(parts[0]);
    else tail.append(parts[0]);
    for (const part of parts.slice(1)) box.append(h('div.tl-think-line', part));
    if (box.childElementCount > keep) {
      const before = box.scrollHeight;
      while (box.childElementCount > keep) box.firstElementChild?.remove();
      const at = this.rollAt ?? { pos: box.scrollTop, vel: 0 };
      this.rollAt = { pos: Math.max(0, at.pos - (before - box.scrollHeight)), vel: at.vel };
      box.scrollTop = this.rollAt.pos;
    }
    this.follow();
  }

  /** 往底下追：一帧走一截，到了停；这一块不在页面上了也停。少动画的直接到底。 */
  follow() {
    if (this.rollFrame) return;
    const box = this.window;
    const reduced = typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;
    const tau = reduced ? 0 : res.timeline.roll_ms;
    let last = performance.now();
    const step = (now) => {
      this.rollFrame = 0;
      if (!box.isConnected || box.hidden) return;
      const target = box.scrollHeight - box.clientHeight;
      this.rollAt = spring(this.rollAt ?? { pos: box.scrollTop, vel: 0 }, target, now - last, tau);
      last = now;
      box.scrollTop = this.rollAt.pos;
      if (this.rollAt.pos !== target || this.rollAt.vel !== 0) this.rollFrame = requestAnimationFrame(step);
    };
    this.rollFrame = requestAnimationFrame(step);
  }

  /**
   * 点开的思考全文：一行一块，来了新字只往最后一行里接、有换行往后添新行，前面的行不动（蓝图「性能」：原来每来一段字把
   * 全文整块换掉，浏览器每一帧从头排，想得越长越卡）。字不是接着原来的（撤销、重来）才整个重画。
   * @param {string} text
   */
  thinkText(text) {
    if (!this.lines || !text.startsWith(this.linesText)) {
      this.lines = h('div.tl-think-lines');
      this.linesText = '';
      replace(this.body, this.lines);
    }
    const added = text.slice(this.linesText.length);
    if (!added) return;
    this.linesText = text;
    const parts = added.split('\n');
    const tail = /** @type {HTMLElement|null} */ (this.lines.lastElementChild);
    if (tail) tail.firstChild ? /** @type {Text} */ (tail.firstChild).appendData(parts[0]) : tail.append(parts[0]);
    else this.lines.append(h('div.tl-think-line', parts[0]));
    for (const part of parts.slice(1)) this.lines.append(h('div.tl-think-line', part));
  }

  /** 工具：状态、命令本身、点开的细节。 */
  updateTool(step, r, open, spinning) {
    this.subject.textContent = r.subject ?? '';
    this.subject.classList.toggle('is-mono', r.mono);
    this.subject.hidden = !r.subject;
    this.said.textContent = r.said ?? '';
    this.said.hidden = !r.said;
    const diff = r.diff ? `+${r.diff.added} -${r.diff.removed}` : '';
    if (this.diff.dataset.sig !== diff) {
      this.diff.dataset.sig = diff;
      replace(this.diff, r.diff ? [h('span.tl-added', `+${r.diff.added}`), ' ', h('span.tl-removed', `-${r.diff.removed}`)] : []);
    }
    this.diff.hidden = !r.diff;
    if (this.peek) {
      const text = messagePeek(step);
      setText(/** @type {HTMLElement} */ (this.peek.lastChild), text);
      this.peek.hidden = !text;
    }
    // 在跑的转一个圈；排着队的一个暗的点；做完的什么都不写
    const status = spinning && step.state === 'running' ? 'spin' : step.state === 'running' ? 'queued' : '';
    if (this.status.dataset.state !== status) {
      this.status.dataset.state = status;
      if (status === 'spin') replace(this.status, icon('loader-circle'));
      else replace(this.status, status === 'queued' ? '·' : '');
    }
    const lines = commandLines(step);
    const sig = JSON.stringify(lines);
    if (sig !== this.drawnCommand) {
      this.drawnCommand = sig;
      replace(this.command, lines ? [...lines.lines.map((l) => h('div.tl-cline', l)), lines.more ? h('div.tl-more', '⋮') : null] : []);
    }
    this.command.hidden = !lines;
    this.drawThumbs(step, open);
    if (!open) return;
    const body = JSON.stringify([step.output, step.status, step.args, step.said]);
    if (body === this.drawnBody) return;
    this.drawnBody = body;
    /** @type {[any, HTMLElement][]} 差异卡片：问到核心算好的差异以后换掉 */
    const cards = [];
    replace(this.body, details(step).map((s) => {
      if (s.kind === 'diff') {
        const card = diffCard(s);
        cards.push([s, card]);
        return card;
      }
      return s.kind === 'images' ? this.imagesNode(s) : h('div.tl-detail', h('div.tl-label', s.label), h('pre', s.text));
    }));
    if (cards.length) this.fullDiff(step, cards);
  }

  /**
   * 编辑、写入做完了：问核心这次调用真改了什么（`view.detail`，完整的差异带行号），照路径换掉照参数比的那张卡片；算不出的写为什么，
   * 进回收站的写一句。问不到的（旧核心、没改成）照参数比的留着。
   * @param {any} step @param {[any, HTMLElement][]} cards
   */
  fullDiff(step, cards) {
    if (!step.callId || step.end == null || !this.where.detail) return;
    this.where.detail(step.callId).then((got) => {
      const files = got?.files ?? [];
      for (const [s, card] of cards) {
        const file = files.find((f) => samePath(f.path, s.path)) ?? (files.length === 1 && cards.length === 1 ? files[0] : null);
        if (!file || !card.isConnected) continue;
        card.replaceWith(file.action === 'trash' ? noteCard(s, t('timeline.trashed'))
          : file.skipped ? noteCard(s, t(`timeline.diff_skipped.${file.skipped}`))
            : diffCard({ ...s, diff: fromUnified(file.diff ?? []) }));
      }
    });
  }

  /** 收着时预览区的小图（`result_image_thumb`）：点开了收掉，换成细节里的；图变了才重画。 */
  drawThumbs(step, open) {
    const images = step.images ?? [];
    this.thumbs.hidden = open || !images.length || !this.where.session;
    const sig = images.map((img) => img.blob).join(',');
    if (this.thumbs.hidden || this.thumbs.dataset.sig === sig) return;
    this.thumbs.dataset.sig = sig;
    const row = /** @type {HTMLElement} */ (this.imagesNode({ label: '', images }).lastChild);
    row.style.setProperty('--media-max', `${res.layout.result_image_thumb}px`);
    replace(this.thumbs, row);
  }

  /** 结果里的图：小一点（`result_image_max`），照核心收下的 blob 取，点开是灯箱（蓝图「图片」第 2 条）。 */
  imagesNode(s) {
    const name = String(this.step.parsed?.file_path ?? '').split('/').pop() || undefined;
    return h('div.tl-detail', s.label ? h('div.tl-label', s.label) : null,
      h('div.tl-images', { style: `--media-max: ${res.layout.result_image_max}px` }, s.images.map((img) =>
        // 不照记的宽高先占地方：记的是原图的，blob 可能缩过、比例对不上，框会多出一截空（照图自己的比例画）
        imageCard({ url: blobUrl(img.blob, img.media_type), name, lightbox: this.where.lightbox }))));
  }

  /** 节点里的图标：变了才换（在想的原子图标在呼吸，别打断）。 */
  setIcon(name, spin) {
    if (name === this.iconName) return;
    this.iconName = name;
    const svg = icon(name);
    if (spin) svg.classList.add('is-spin');
    replace(this.node, svg);
  }
}

/** 用时那一格：做完的写定的字；在走的记下从哪一刻起、怎么写，由 `Ticker` 走表。 */
function setTime(el, r, preparing) {
  el.hidden = !r.took && !r.timer;
  el.classList.toggle('tl-timer', !!r.timer);
  if (r.timer) {
    el.dataset.since = String(r.timer.since);
    el.dataset.format = r.timer.format;
    el.dataset.lead = preparing ? ' · ' : '';
  } else {
    delete el.dataset.since;
    el.textContent = r.took ?? '';
  }
}

/**
 * 差异卡片（照旧版 `diff.js:139-200`）：头一行「修改」「新建」、路径、`+N −N`；下面一行一行，加的 `+`、删的 `−`，
 * 两处之间一行 `⋯`。
 */
function diffCard(s) {
  const marks = { added: '+', removed: '−', keep: ' ' };
  // 行号：核心算好的差异、写入照参数比的有；编辑照参数比的不知道在第几行，这一列空着
  const numbered = s.diff.lines.some((l) => l.number != null);
  return h(`div.diff-file${numbered ? '.is-numbered' : ''}`,
    h('div.diff-file-head',
      h(`span.diff-op${s.created ? '.is-add' : ''}`, s.op),
      h('span.diff-path', s.path),
      h('span.diff-stat', h('b.diff-stat-add', `+${s.diff.added}`), ' ', h('b.diff-stat-del', `−${s.diff.removed}`))),
    h('div.diff-lines', s.diff.lines.map((l) => (l.mark === 'gap'
      ? h('div.diff-hunk', '⋯')
      : h(`div.diff-line.is-${l.mark}`, numbered ? h('span.diff-no', l.number ?? '') : null, h('span.diff-gutter', marks[l.mark]), h('span.diff-text', l.text))))));
}

/** 没有差异可画的那张卡片（进回收站的、核心算不出的）：头一行照旧，下面一句为什么。 */
function noteCard(s, text) {
  return h('div.diff-file', h('div.diff-file-head', h(`span.diff-op${s.created ? '.is-add' : ''}`, s.op), h('span.diff-path', s.path)), h('p.diff-note', text));
}

/** 点击：拖选了字的不算点（字照常能拖选）。 */
export function guard(fn) {
  return (/** @type {Event} */ e) => {
    if (String(getSelection() ?? '')) return;
    e.stopPropagation();
    fn();
  };
}

/** 字变了才写（流式的参数一个字一个字来，别每一下都动 DOM）。 */
function setText(el, text) {
  if (el.textContent !== text) el.textContent = text;
}
