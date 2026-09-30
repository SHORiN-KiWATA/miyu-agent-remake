// @ts-check
//! 运行状态行挑哪个词、点几个（蓝图 `web.md`「运行状态行」，规矩照 `tui.md`「运行状态行和排队的消息」第 1–2 条，
//! 做法照 TUI 演示的 `pulse.rs`、`ui/status.rs`；软件包 `pulse`）。纯的：时刻、随机数都从外面给，测试里定死。
//!
//! 词是中性的，按这一轮跑了多久分档（`resources/pulse.json`）。事件来得很快，一串算一阵，最后一件之后安静够了
//! （`quiet_ms`）这一阵才算完，完了换一个词；一个词至少停一会儿（`dwell_ms`，换上时随机定），没停够等停够；
//! 没有事件时待久了也换（`idle_ms`，随机）。跨档时同样先停够。不连着重复刚才那个，新的一轮从头挑。

/** @typedef {{after: number, words: string[]}} Tier 这一轮跑了 `after` 秒以后用这些词 */
/** @typedef {{dwell_ms: number[], quiet_ms: number, idle_ms: number[], tiers: Tier[]}} Words 词库；范围是 `[最小, 最大]` 毫秒 */
/** @typedef {{id: string, start: number}} Turn 哪一轮（会话加回合，变了就是新的一轮）、什么时候开始的（毫秒） */

/** 正在写的那个词，和换词要记的几样。 */
export class Pulse {
  /** @param {() => number} random `[0, 1)` 的随机数：挑词、定时长用；测试里给定死的 */
  constructor(random = Math.random) {
    this.random = random;
    /** 哪一轮：变了就从头挑。 */
    this.turn = /** @type {string|null} */ (null);
    this.current = '';
    /** 这个词什么时候换上的（毫秒）：流光照它从头扫，点照它轮，停没停够也照它算。 */
    this.shown = /** @type {number|null} */ (null);
    this.dwell = 0;
    this.idle = 0;
    /** 最后看到的事件记号，和它变的时刻：一阵事件还在来还是已经安静下来。 */
    this.beat = /** @type {unknown} */ (null);
    this.lastEvent = /** @type {number|null} */ (null);
    /** 有一阵事件完了，还没换过词。 */
    this.due = false;
  }

  /**
   * 这一刻写哪个词。`beat` 是这一轮到现在出过的事的记号（`beatOf`），变了就是来了新的事。
   * @param {Turn} turn
   * @param {unknown} beat
   * @param {number} now 毫秒
   * @param {Words} words
   */
  word(turn, beat, now, words) {
    if (this.turn !== turn.id) {
      this.turn = turn.id;
      this.current = '';
      this.beat = beat;
      this.lastEvent = null;
      this.due = false;
    }
    if (beat !== this.beat) {
      this.beat = beat;
      this.lastEvent = now;
    }
    // 最后一件事之后安静够了：这一阵完了，记一笔该换了
    if (this.lastEvent != null && now - this.lastEvent >= words.quiet_ms) {
      this.lastEvent = null;
      this.due = true;
    }
    const pool = poolOf(words, now - turn.start);
    const held = this.shown == null ? Infinity : now - this.shown;
    const settled = held >= this.dwell;
    const change = this.current === '' || (settled && (this.due || !pool.includes(this.current))) || held >= this.idle;
    if (change) {
      this.pick(pool);
      this.shown = now;
      this.due = false;
      this.dwell = this.between(words.dwell_ms);
      this.idle = this.between(words.idle_ms);
    }
    return this.current;
  }

  /** 在 `pool` 里随机挑一个，不挑刚才那个（只有一个词的除外）。 */
  pick(pool) {
    const others = pool.filter((w) => w !== this.current);
    const choices = others.length ? others : pool;
    if (choices.length) this.current = choices[Math.floor(this.random() * choices.length)];
  }

  /** `[最小, 最大]` 毫秒里随机一个时长，两头都取得到。 */
  between([low, high]) {
    const lo = Math.min(low, high);
    return lo + Math.floor(this.random() * (Math.abs(high - low) + 1));
  }
}

/** 这一轮跑了 `ran` 毫秒该用哪一档的词：`after` 不超过它的最后一档；都超过的用第一档。 */
function poolOf(words, ran) {
  const secs = Math.floor(ran / 1000);
  const tier = [...words.tiers].reverse().find((t) => t.after <= secs) ?? words.tiers[0];
  return tier?.words ?? [];
}

/** 占几格：中日韩的字、全角的两格，别的一格（和 TUI 的 `unicode-width` 对得上词库里的字）。 */
export function columns(text) {
  let n = 0;
  for (const ch of text) n += /[ᄀ-ᅟ⺀-꓏가-힣豈-﫿︰-﹏＀-｠￠-￦]/u.test(ch) ? 2 : 1;
  return n;
}

/** 词库里最宽的词：用时照它留位置，换了长短不一的词不左右跳（`tui.md` 第 2 条）。 */
export function widest(words) {
  let best = '';
  for (const w of words.tiers.flatMap((t) => t.words)) if (columns(w) > columns(best)) best = w;
  return best;
}

/**
 * 点写几个：一趟亮光（`sweep` 秒）正好轮一圈 `.`、`..`、`...`，一个点停 `sweep / count` 秒。`t` 是这个词换上以后
 * 过了几秒（换了词和亮光一起从一个点重来）。照 TUI 的 `ui/status.rs` 的 `swept`。
 */
export function dotCount(t, sweep, count) {
  const n = Math.max(1, count);
  const phase = sweep > 0 ? (((t % sweep) + sweep) % sweep) / sweep : 0;
  return Math.min(Math.floor(phase * n), n - 1) + 1;
}

/**
 * 这一轮出过的事的记号。照 TUI，算一件事的只有步与步的交界：开了新的一步（在收的回复多开一块，思考、调工具、
 * 开始写回答都是开一块）、一块收全了（想完、参数写完）、落了盘的事件（回复落盘、工具出了结果、一轮结束）。
 * 接着往一块里写字不算，不然她写回答时一直算在来事、词永远换不了。只比变没变，不比大小（回复落盘时在收的那一份
 * 扔掉，数会往回走）。
 * @param {any[]} events 这个会话的持久事件
 * @param {{seen: number, blocks: ({text: string, done: boolean}|undefined)[]}|null} live 在收的那一次回复（`core/store.js`）
 */
export function beatOf(events, live) {
  if (!live) return `${events.length}`;
  const opened = live.blocks.filter(Boolean).length;
  const done = live.blocks.filter((b) => b?.done).length;
  return `${events.length}|${live.seen}|${opened}|${done}`;
}
