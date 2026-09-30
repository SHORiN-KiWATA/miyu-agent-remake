// @ts-check
//! 吉祥物肚子上的灯（软件包 `mascot`，蓝图 `web.md`「吉祥物」第 7 条；2026-09-30 项目主人定：那个圈当作灯，会闪）：她在回答时一闪
//! 一闪；有后台任务在跑、她没在回答时隔一阵亮一下；一轮结束快闪几下；别的时候灭着。系统设了减少动画的不闪，回答、有后台任务时一直
//! 亮。亮灭只在变的那一刻叫一次 `changed`（排好下一次变的时刻），不一帧一帧地推（「空闲就是真的空闲」）。

/**
 * @typedef {{busy_on_ms: number, busy_off_ms: number, jobs_every_ms: number, jobs_on_ms: number, flash_ms: number, flashes: number}} LampConfig
 * @typedef {'busy'|'jobs'|'off'} Mode 在回答、有后台任务、灭着
 */

/**
 * 这一刻亮不亮：`t` 是进这个样子以后过了多久（毫秒）。
 * @param {Mode} mode
 * @param {number} t
 * @param {LampConfig} c
 * @param {boolean} reduced 减少动画
 */
export function lit(mode, t, c, reduced) {
  if (mode === 'off') return false;
  if (reduced) return true;
  if (mode === 'busy') return t % (c.busy_on_ms + c.busy_off_ms) < c.busy_on_ms;
  return t % c.jobs_every_ms < c.jobs_on_ms;
}

/** 离下一次亮灭变还有多久（毫秒）；不会再变的是 `null`。 */
function nextIn(mode, t, c, reduced) {
  if (mode === 'off' || reduced) return null;
  const [on, period] = mode === 'busy' ? [c.busy_on_ms, c.busy_on_ms + c.busy_off_ms] : [c.jobs_on_ms, c.jobs_every_ms];
  const ph = t % period;
  return ph < on ? on - ph : period - ph;
}

export class Lamp {
  /**
   * @param {LampConfig} config
   * @param {() => boolean} reduced 减少动画
   * @param {() => void} changed 亮灭变了
   */
  constructor(config, reduced, changed) {
    this.config = config;
    this.reduced = reduced;
    this.changed = changed;
    /** @type {Mode} */
    this.mode = 'off';
    this.since = 0;
    /** 快闪：从什么时候起、到什么时候 */
    this.flashAt = 0;
    this.flashUntil = 0;
    this.on = false;
    this.timer = /** @type {any} */ (0);
  }

  /** 换样子（一样的不动）。 */
  set(mode) {
    if (mode === this.mode) return;
    this.mode = mode;
    this.since = performance.now();
    this.tick();
  }

  /** 快闪几下（一轮结束）；减少动画的不闪。 */
  flash() {
    if (this.reduced()) return;
    const c = this.config;
    this.flashAt = performance.now();
    this.flashUntil = this.flashAt + c.flash_ms * c.flashes * 2;
    this.tick();
  }

  /** 照现在算亮不亮，变了告诉外面；排好下一次变的时刻。 */
  tick() {
    clearTimeout(this.timer);
    const c = this.config;
    const now = performance.now();
    const flashing = now < this.flashUntil;
    const on = flashing ? Math.floor((now - this.flashAt) / c.flash_ms) % 2 === 0 : lit(this.mode, now - this.since, c, this.reduced());
    if (on !== this.on) {
      this.on = on;
      this.changed();
    }
    const next = flashing ? Math.min(c.flash_ms - ((now - this.flashAt) % c.flash_ms), this.flashUntil - now) : nextIn(this.mode, now - this.since, c, this.reduced());
    if (next != null) this.timer = setTimeout(() => this.tick(), Math.max(1, next));
  }

  /** 停用了：不再排。 */
  destroy() {
    clearTimeout(this.timer);
  }
}
