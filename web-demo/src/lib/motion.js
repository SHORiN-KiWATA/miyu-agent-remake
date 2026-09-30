// @ts-check
//! 进场、退场（蓝图 `web.md`「动效」）：进场是 CSS 动画，节点露出来时自己放；退场先加 `is-leaving`（CSS 里写它退场的样子），
//! 动画走完再藏起来或拿掉。退到一半又要出来的，停掉退场，直接回到出来的样子；系统设了减少动画的、这个节点没写退场动画的，
//! 马上收。动画事件没来的（节点被别的藏了、标签页在后台），照算出来的时长兜底收。
//!
//! 在流里占着地方的一块（输入框上面的待办、运行状态行）不走这一套，用 `unfold`：CSS 过渡，收到一半又要出来的从当时的高度倒回去。

const LEAVING = 'is-leaving';
/** 动画事件晚到的余量（毫秒）：一帧多一点，兜底用 */
const SLACK_MS = 50;
/** @type {WeakMap<Element, () => void>} 正在退场的：怎么停掉它 */
const leaving = new WeakMap();

/**
 * 一组动画要走多久（毫秒）：`animation-duration`、`animation-delay` 的计算值，几段一一对上，取时长加延迟最长的。
 * @param {string} durations 比如 `0.12s, 160ms`
 * @param {string} delays
 */
export function span(durations, delays) {
  const ms = (v) => (v.trim().endsWith('ms') ? parseFloat(v) : parseFloat(v) * 1000) || 0;
  const d = durations.split(',').map(ms);
  const l = delays.split(',').map(ms);
  return Math.max(0, ...d.map((x, i) => x + (l[i] ?? l[0] ?? 0)));
}

/** 系统设了减少动画。 */
const reduced = () => typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;

/**
 * 退场：加 `is-leaving`，自己的动画走完调 `done`（拿掉、藏起来）。没有退场动画的、减少动画的，马上调。
 * @param {HTMLElement} el
 * @param {() => void} done
 */
export function leave(el, done) {
  leaving.get(el)?.();
  if (!el.isConnected || el.hidden || reduced()) {
    done();
    return;
  }
  el.classList.add(LEAVING);
  const style = getComputedStyle(el);
  const total = style.animationName === 'none' ? 0 : span(style.animationDuration, style.animationDelay);
  if (!total) {
    el.classList.remove(LEAVING);
    done();
    return;
  }
  let over = false;
  const finish = (/** @type {boolean} */ ok) => {
    if (over) return;
    over = true;
    clearTimeout(timer);
    el.removeEventListener('animationend', onEnd);
    leaving.delete(el);
    el.classList.remove(LEAVING);
    if (ok) done();
  };
  const onEnd = (/** @type {AnimationEvent} */ e) => { if (e.target === el) finish(true); };
  el.addEventListener('animationend', onEnd);
  const timer = setTimeout(() => finish(true), total + SLACK_MS);
  leaving.set(el, () => finish(false));
}

/** 藏起来：先走退场。 */
export function hide(el) {
  if (el.hidden && !leaving.has(el)) return;
  leave(el, () => { el.hidden = true; });
}

/** 露出来：正在退场的停掉，直接回到出来的样子（进场动画由 CSS 在露出来时放）。 */
export function show(el) {
  leaving.get(el)?.();
  el.hidden = false;
}

/**
 * 占着地方的一块出来、收回去：CSS 照 `is-on` 把高度从 0 长出来、收回 0（`base.css` 的 `.unfold`、`.unfold-inner`）。是过渡不是
 * 动画，收到一半又要出来的从当时的高度倒回去。收着的里面的东西留着（收的时候还看得到），不能点、读屏不读。
 * @param {HTMLElement} el
 * @param {boolean} on
 */
export function unfold(el, on) {
  el.classList.toggle('is-on', on);
  el.inert = !on;
  el.setAttribute('aria-hidden', String(!on));
}
