// @ts-check
//! 吉祥物的脸（软件包 `mascot`，蓝图 `web.md`「吉祥物」第 5–6 条；照 TUI `tui-demo/src/mascot/idle.rs`）：转头照目标缓动过去、
//! 眨眼（有时连眨两下）、耳朵往外抖一下、摔重了闭眼晕一会儿；嘴张多大也缓着变：打哈欠（张到最大、闭眼）。只记状态、
//! 一帧一帧推，什么时候做由 `behavior.js` 定。

/** 缓动：过 `dt` 毫秒，离目标的差剩 `0.5^(dt/半衰期)` */
export const ease = (now, target, dt, half) => target + (now - target) * 0.5 ** (dt / half);

export class Face {
  /** @param {any} config 这个包的设置项 */
  constructor(config) {
    this.config = config;
    /** 这一帧的样子 */
    this.pose = /** @type {import('./model.js').Pose} */ ({ yaw: 0, pitch: 0, blink: false, ear: 0, stride: 0, crouch: 0, mouth: 0, turn: 0, lamp: false });
    /** 转头的目标（度）和缓动的半衰期 */
    this.aim = { yaw: 0, pitch: 0, half: config.gaze.half_life_ms };
    /** 眨眼到什么时候、连眨的第二下什么时候、晕到什么时候（闭着眼） */
    this.blinkUntil = 0;
    this.blinkAgain = 0;
    this.dizzyUntil = 0;
    /** 抖耳朵：什么时候开始、往外歪多少 */
    this.twitchAt = /** @type {number|null} */ (null);
    this.twitchTilt = 0;
    /** 嘴：这一帧张多大；一会儿张成多大、到什么时候、缓多快 */
    this.mouth = 0;
    this.gaping = /** @type {{level: number, until: number, half: number}|null} */ (null);
  }

  /** 嘴张成 `level` 张 `ms` 毫秒再回去，照 `half` 缓着变。 */
  gape(level, ms, half = this.config.mouth.half_life_ms) {
    this.gaping = { level, until: performance.now() + ms, half };
  }

  /** 打哈欠：嘴慢慢张到最大、闭上眼，再合上。 */
  yawn() {
    const m = this.config.mouth;
    this.gape(m.yawn, m.yawn_ms, m.yawn_half_life_ms);
    this.blinkUntil = performance.now() + m.yawn_ms;
  }

  /** 转到 `yaw`、`pitch`，照 `half` 缓动过去。 */
  look(yaw, pitch, half = this.config.gaze.half_life_ms) {
    this.aim = { yaw, pitch, half };
  }

  /** 闭眼 `ms` 毫秒；`again` 的过一会儿再眨一下。 */
  blink(ms, again = false) {
    const now = performance.now();
    this.blinkUntil = now + ms;
    if (again) this.blinkAgain = now + this.config.idle.blink_ms[1] + this.config.idle.double_gap_ms;
  }

  /** 耳朵往外抖一下，歪 `tilt`。 */
  twitch(tilt) {
    this.twitchAt = performance.now();
    this.twitchTilt = tilt;
  }

  /** 摔晕了：闭眼 `ms` 毫秒。 */
  daze(ms) {
    this.dizzyUntil = performance.now() + ms;
  }

  /** 推一帧：转头、眨眼、抖耳朵、嘴；交回还在不在动。 */
  tick(dt, now) {
    const p = this.pose;
    const c = this.config;
    let moving = false;
    p.yaw = ease(p.yaw, this.aim.yaw, dt, this.aim.half);
    p.pitch = ease(p.pitch, this.aim.pitch, dt, this.aim.half);
    if (Math.abs(p.yaw - this.aim.yaw) < 0.2 && Math.abs(p.pitch - this.aim.pitch) < 0.2) {
      p.yaw = this.aim.yaw;
      p.pitch = this.aim.pitch;
    } else moving = true;
    if (this.blinkAgain && now >= this.blinkAgain) {
      this.blinkAgain = 0;
      this.blinkUntil = now + c.idle.blink_ms[0];
    }
    p.blink = now < this.blinkUntil || now < this.dizzyUntil;
    if (p.blink || this.blinkAgain) moving = true;
    const gaping = this.gaping && now < this.gaping.until ? this.gaping : null;
    if (!gaping) this.gaping = null;
    const want = gaping?.level ?? c.mouth.rest;
    this.mouth = ease(this.mouth, want, dt, gaping?.half ?? c.mouth.half_life_ms);
    if (Math.abs(this.mouth - want) < 0.02) this.mouth = want;
    else moving = true;
    if (gaping) moving = true;
    p.mouth = this.mouth;
    if (this.twitchAt != null) {
      const k = (now - this.twitchAt) / c.idle.twitch_ms;
      if (k >= 1) {
        this.twitchAt = null;
        p.ear = 0;
      } else {
        p.ear = Math.sin(Math.PI * k) * this.twitchTilt;
        moving = true;
      }
    }
    return moving;
  }
}
