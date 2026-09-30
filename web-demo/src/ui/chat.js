// @ts-check
//! 对话区（蓝图 `web.md`「对话区」，照旧版 `styles.css:1402-2442`）：你的话是靠右的气泡；她的话一轮一块，
//! 头上头像和名字，下面的回答、时间线、收尾那一行缩进在名字底下。一轮开始就有她的头，还没出字时下面是三个球。
//!
//! 每一块、每一条按编号记着画好的节点：内容没变的不重画（流式的字一段段接上来时，只有在收的那一条重画，
//! 选中的字、滚到哪都不丢）。滚到哪由 `follow.js` 管：跟着最新的、不往回退、长回答停住、点开时钉住被点的那一行。

import { h } from './dom.js';
import { res } from '../util/res.js';
import { SegmentView, Ticker, waitingNode } from './timeline.js';
import { renderMarkdown } from '../markdown/render.js';
import { richHooks } from './rich.js';
import { Follow } from './follow.js';
import { userNode, endNode } from './said.js';
import { noteNode } from './notes.js';
import { group } from '../model/group.js';

export class Chat {
  /**
   * @param {string|null} home 家目录：时间线的标题里写成 `~`
   * @param {{say: (text: string, good?: boolean) => void}} notice 提示（回答里复制了代码、路径）
   * @param {import('./said.js').Actions} on 你的话、她一轮末尾的按钮做什么（`said.js`）
   * @param {import('./rich.js').Ext} ext 软件包接进来的：挂载位、现在的灯箱（回答里的代码块、图片照它）
   */
  constructor(home, notice, on, ext) {
    this.on = on;
    this.ext = ext;
    /** 上一次画的条目：挂的东西变了照它重画 */
    this.last = /** @type {any[]|null} */ (null);
    this.home = home;
    this.say = (/** @type {string} */ text, /** @type {boolean|undefined} */ good) => notice.say(text, good);
    /** 看着的这个会话在哪（`ui/rich.js`）：取本机文件的地址要；换了会话由 `setWhere` 换。 */
    this.where = /** @type {import('./rich.js').Where} */ ({ session: null, home, cwd: null, lightbox: () => ext?.lightbox?.() });
    /** 画回答时带着的（`markdown/render.js`）：提示；扩展点（图、卡片）照一条回答的范围取。 */
    this.markdown = { say: this.say, hooks: richHooks(this.where, this.say, ext) };
    this.list = h('div.timeline');
    /** 正文末尾、最后一轮下面（挂载位 `chat.tail`：确认和提问了结以后留的）：一直是正文那一列的最后一个，画的时候不动它 */
    this.tail = h('div.chat-tail');
    this.list.append(this.tail);
    /** 内容变短时垫在底下的空白（见开头）。 */
    this.spacer = h('div.chat-spacer');
    this.el = h('div.chat-scroll', this.list, this.spacer);
    this.scroll = new Follow(this.el, this.list, this.spacer);
    /** 看着你说的话的（软件包 rail 这类，经服务 `chat`）：每画一次交一份；新来的先拿到现在的 */
    this.promptWatchers = new Set();
    /** @type {Array<{key: string, text: string, node: HTMLElement}>} */
    this.prompts = [];
    // 人点开、收起时间线（`timeline.js`、`steps.js` 发的）：被点的那一行钉在原地
    this.list.addEventListener('tl-toggle', (e) => this.scroll.toggled(/** @type {Element} */ (e.target)));
    this.ticker = new Ticker(this.list);
    /** 换了会话以后画过一次了：之后新来的时间线的步淡入，第一次画的（读回来的历史）不淡入。 */
    this.settled = false;
    /** @type {Map<string, {node: HTMLElement, content: HTMLElement|null, sig: string, items: Map<string, {node: HTMLElement, sig: string, view?: SegmentView}>}>} */
    this.blocks = new Map();
  }

  /**
   * 看着的会话在哪：会话编号、工作目录变了，回答里的本机地址跟着换（重画的时候才用上新的）。
   * @param {string|null} session
   * @param {string|null} cwd
   */
  setWhere(session, cwd) {
    if (this.where.session === session && this.where.cwd === cwd) return;
    this.where = { session, home: this.home, cwd, lightbox: () => this.ext?.lightbox?.() };
    this.markdown = { say: this.say, hooks: richHooks(this.where, this.say, this.ext) };
  }

  /** 正文末尾（`chat.tail`）来了新的：回到跟着最新的，露出它（确认和提问刚了结，蓝图「确认和提问」第 6 条）。 */
  reveal() {
    this.scroll.toLatest();
    this.scroll.anchor();
  }

  /** 换了会话：从头排，跟着最新的；点过的展开收起跟着节点一起扔掉（编号照回合，别的会话也有）。 */
  reset() {
    this.blocks.clear();
    this.list.replaceChildren(this.tail);
    this.scroll.toLatest(true);
    this.settled = false;
  }

  /** 挂进来的画法变了（装了、停了 mermaid 这类包）：照上一次的条目整个重画，滚到哪留着，时间线不淡入。 */
  redraw() {
    if (!this.last) return;
    const top = this.el.scrollTop;
    this.blocks.clear();
    this.list.replaceChildren(this.tail);
    this.markdown = { say: this.say, hooks: richHooks(this.where, this.say, this.ext) };
    this.settled = false;
    this.render(this.last);
    this.el.scrollTop = top;
  }

  /** 看着你说的话：先交现在的一份，以后每画一次交一份；交回怎么不看。 */
  onPrompts(fn) {
    this.promptWatchers.add(fn);
    fn(this.prompts);
    return () => this.promptWatchers.delete(fn);
  }

  /** 有能重做、编辑的那一句（开最新那一轮的、你说的）。 */
  hasLatest() {
    return !!this.list.querySelector('.user-message.is-latest');
  }

  /** `/edit`：滚到开最新那一轮的那一句（停在视口正中，不再跟着最新的），打开编辑（和点它下面的「编辑」一样）。 */
  editLatest() {
    const node = /** @type {HTMLElement|undefined} */ ([...this.list.querySelectorAll('.user-message.is-latest')].at(-1));
    if (!node) return;
    this.scroll.leave();
    node.scrollIntoView({ block: 'center' });
    /** @type {HTMLElement|null} */ (node.querySelector('.is-edit'))?.click();
  }

  /** 重做开始：记住能重做的那一句（开最新那一轮的）在屏幕上的位置（`follow.js` 的 `keep`）。 */
  keepLatest() {
    const node = [...this.list.querySelectorAll('.user-message.is-latest')].at(-1);
    if (node) this.scroll.keep(node);
  }

  /** 在回答：编辑、重做的按钮藏起来（`said.js`）；一轮结束了，底下还垫着的空白收掉（`follow.js` 的 `release`）。 */
  setRunning(yes) {
    const was = this.el.classList.contains('is-running');
    this.el.classList.toggle('is-running', yes);
    if (was && !yes) this.scroll.release();
  }

  /** @param {any[]} items 正文的条目（`model/transcript.js`） */
  render(items) {
    this.last = items;
    const keep = new Set();
    let prev = null;
    const last = items.at(-1);
    // 最新的一轮：编辑、重做只有它有；不是你开的（子代理的会话里派它的会话发来的、别人说的）没有（核心只重做人开的那一轮）
    const opener = items.findLast((it) => it.type === 'user');
    const latest = opener && this.mine(opener) ? opener.turn ?? null : null;
    /** @type {Array<{key: string, text: string, node: HTMLElement}>} 你说的话，照先后：交给看着它的（跳转条） */
    const prompts = [];
    /** @type {HTMLElement[]} 你新说的：画进页面以后再照它放视口 */
    const arrivals = [];
    /** @type {HTMLElement|null} 看着的时候来的「上下文已清空」 */
    let cleared = null;
    // 最后那段正文：画的时候记下来（`reconcile`），长过视口时照它停住、停住以后照它钉着
    this.scroll.reply = null;
    for (const block of group(items)) {
      const own = block.kind === 'user' && this.mine(block.item);
      const freshNote = this.settled && block.kind === 'note' && block.item.compaction === 'clear' && !this.blocks.has(block.key);
      // 你新说的一句（不是刚打开会话时读回来的）：回到最底下，照最新的露；别人说的不拽视口
      const fresh = this.settled && own && !this.blocks.has(block.key);
      const node = this.block(block);
      // 你新说的一句：回到最底下；重做的落回原来的位置（`follow.js`）。等它画进页面再放
      if (fresh) arrivals.push(node);
      // 你的话里只有开这一轮的那一句能编辑（排进来的、别人说的没有，蓝图「排队的消息」第 5 条、「不是你说的话」）
      const mine = own && block.item.opens;
      node.classList.toggle('is-latest', latest != null && (block.kind === 'user' ? mine && block.item.turn : block.turn) === latest);
      if (block.kind === 'user') prompts.push({ key: block.key, text: block.item.text, node });
      if (freshNote) cleared = node;
      keep.add(block.key);
      const want = prev ? prev.nextSibling : this.list.firstChild;
      if (want !== node) this.list.insertBefore(node, want);
      prev = node;
    }
    for (const [key, rec] of this.blocks) {
      if (keep.has(key)) continue;
      rec.node.remove();
      this.blocks.delete(key);
    }
    for (const node of arrivals) this.scroll.arrived(node);
    // 看着的时候清空了：那一行顶到最上面（照终端的 Ctrl+L）
    if (cleared) this.scroll.toTop(cleared);
    // 停住以后她开始了下一步（最后一条是时间线的一段）：回到最底下接着跟
    if (last?.type === 'steps' && !last.finished) this.scroll.stepped();
    this.scroll.anchor();
    this.prompts = prompts;
    for (const fn of [...this.promptWatchers]) fn(prompts);
    this.ticker.update();
    this.settled = true;
  }

  /** 这一句是不是你（这个页面登录成的账号）说的；不知道账号的，人说的都算你的。 */
  mine(item) {
    const s = item.speaker;
    return !s || (s.kind === 'person' && (this.ext.account == null || s.account === this.ext.account));
  }

  /** 一块：你的（别人的）一句话、她的一轮，或者不挂在她头下的一行。没变的直接用记着的节点。 */
  block(block) {
    const sig = block.kind === 'her' ? `her${block.cont ? '+' : ''}` : JSON.stringify(block.item);
    let rec = this.blocks.get(block.key);
    if (!rec || rec.sig !== sig) {
      const node = block.kind === 'user'
        ? userNode(block.item, this.on, { session: this.where.session, lightbox: this.ext.lightbox, mine: this.mine(block.item), titleOf: this.ext.titleOf })
        : block.kind === 'note' ? noteNode(block.item, this.where, this.markdown) : herNode(!!block.cont);
      rec?.node.replaceWith(node);
      rec = { node, content: node.querySelector('.assistant-content'), sig, items: new Map() };
      this.blocks.set(block.key, rec);
    }
    if (rec.content) reconcile(rec.content, rec.items, block.items, this);
    return rec.node;
  }
}

/**
 * 她的一块里的条目：照编号对上，变了的换掉，没了的去掉。时间线的一段不换：一直是同一个节点，照新的样子改
 * （`timeline.js`），点开的、淡入过的都留着。
 */
function reconcile(parent, known, items, chat) {
  const keep = new Set();
  let prev = null;
  // 回答的范围：回合加这一轮里第几段回答（在收的落了盘编号变了，范围不变，图和卡片接着用，`rich.js`）
  let replies = 0;
  for (const it of items) {
    if (it.type === 'reply') it.scope = `${it.turn}:${replies++}`;
    let rec = known.get(it.key);
    if (it.type === 'steps') {
      if (!rec) {
        const view = new SegmentView(chat.where, chat.settled);
        rec = { node: view.el, sig: '', view };
        known.set(it.key, rec);
      }
      rec.view.update(it);
    } else {
      const sig = JSON.stringify(it);
      if (!rec || rec.sig !== sig) {
        const node = itemNode(it, chat);
        rec?.node.replaceWith(node);
        rec = { node, sig };
        known.set(it.key, rec);
      }
      if (it.type === 'reply') chat.scroll.reply = { key: it.scope, node: rec.node, streaming: it.streaming };
    }
    keep.add(it.key);
    const want = prev ? prev.nextSibling : parent.firstChild;
    if (want !== rec.node) parent.insertBefore(rec.node, want);
    prev = rec.node;
  }
  for (const [key, rec] of known) {
    if (keep.has(key)) continue;
    rec.node.remove();
    known.delete(key);
  }
}

/** 她的一轮：头像和名字，下面是内容；`cont` 的是接着她同一轮的（中间插进来一句话），只有内容。 */
function herNode(cont) {
  const p = res.persona;
  if (cont) return h('article.assistant-message.is-cont', h('div.assistant-content'));
  return h('article.assistant-message',
    h('header.assistant-label', h('img', { src: p.avatar, alt: '' }), h('strong', p.name)),
    h('div.assistant-content'));
}

/**
 * 她的一条：回答、三个球、收尾那一行、夹在中间的回报。时间线的一段见 `reconcile`。
 * 回答照 Markdown 画（`markdown/render.js`）；在收的先补齐半截的写法，字一变整条换掉重画。
 */
function itemNode(it, chat) {
  if (it.type === 'done') return endNode(it, chat.on);
  // 她正在回答时来的回报：夹在她这一块里（上下两段时间线中间）
  if (it.type === 'note') return noteNode(it, chat.where, chat.markdown);
  if (it.type === 'waiting') return waitingNode();
  const node = h('div.reply.markdown-body');
  renderMarkdown(node, it.text, { say: chat.markdown.say, hooks: chat.markdown.hooks(it.scope), streaming: it.streaming });
  return node;
}
