// @ts-check
//! 时间线的一步（蓝图 `web.md`「时间线」，照旧版网页 `app.js:6962-7051`、`8574-8771`、`diff.js`）：思考一行、工具一行，
//! 下面接在想时的窗口、命令本身、点开的细节。
//!
//! 一步是一个一直在的节点，流式的字来了只改里面的字：新来的一步淡入一次，在想的标题的流光不断（重建节点会从头来）。
//! 点开、收起记在这一步里，人点过的照人点的；没点过的照 `timeline.json` 的 `expand`。

import { h, icon, replace } from './dom.js';
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
      this.body = h('div.tl-body');
      this.el = h(`div.tl-step.is-tool${message ? '.is-message' : ''}${fresh ? '.is-new' : ''}`, this.row, this.command, h('div.tl-fold', h('div.tl-fold-inner', this.body)));
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
    if (rolling) this.window.textContent = thinkingTail(step);
    // 收起时字留着，收的动画里还看得到；点开时照最新的字换
    if (open) this.body.textContent = step.text.trim();
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
    if (!open) return;
    const body = JSON.stringify([step.output, step.status, step.args, step.said]);
    if (body === this.drawnBody) return;
    this.drawnBody = body;
    replace(this.body, details(step).map((s) => (s.kind === 'diff' ? diffCard(s)
      : s.kind === 'images' ? this.imagesNode(s)
        : h('div.tl-detail', h('div.tl-label', s.label), h('pre', s.text)))));
  }

  /** 结果里的图：小一点（`result_image_max`），照核心收下的 blob 取，点开是灯箱（蓝图「图片」第 2 条）。 */
  imagesNode(s) {
    const name = String(this.step.parsed?.file_path ?? '').split('/').pop() || undefined;
    const session = this.where.session;
    return h('div.tl-detail', s.label ? h('div.tl-label', s.label) : null,
      h('div.tl-images', { style: `--media-max: ${res.layout.result_image_max}px` }, s.images.map((img) => (session
        ? imageCard({ url: blobUrl(session, img.blob, img.media_type), name, width: img.width, height: img.height, lightbox: this.where.lightbox })
        : null))));
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
  return h('div.diff-file',
    h('div.diff-file-head',
      h(`span.diff-op${s.created ? '.is-add' : ''}`, s.op),
      h('span.diff-path', s.path),
      h('span.diff-stat', h('b.diff-stat-add', `+${s.diff.added}`), ' ', h('b.diff-stat-del', `−${s.diff.removed}`))),
    h('div.diff-lines', s.diff.lines.map((l) => (l.mark === 'gap'
      ? h('div.diff-hunk', '⋯')
      : h(`div.diff-line.is-${l.mark}`, h('span.diff-gutter', marks[l.mark]), h('span.diff-text', l.text))))));
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
