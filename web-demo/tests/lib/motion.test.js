// @ts-check
//! 退场（蓝图 `web.md`「动效」）：一个节点的动画要走多久（照计算好的 `animation-duration`、`animation-delay`，几段取最长的）；
//! 在流里占着地方的一块出来、收回去（`unfold`）；照 CSS 的 `cubic-bezier()` 算曲线；高度对齐到整的屏幕像素（整页放大 1.1、屏幕缩放
//! 以后，一帧一帧缓的小数高度会让贴着下沿的内容上下抖 1 像素：2026-10-02 项目主人觉得切页抖，逐帧截图量出来的）。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { span, unfold, cubicBezier, snapToPixels, parseBezier } from '../../src/lib/motion.js';

test('动画走多久：秒、毫秒都认，几段取最长的（时长加延迟）；没有动画是 0', () => {
  assert.equal(span('0.12s', '0s'), 120);
  assert.equal(span('0.12s, 160ms', '0s, 50ms'), 210);
  assert.equal(span('0s', '0s'), 0);
  assert.equal(span('', ''), 0);
});

/** 假的节点：只有 `unfold` 碰的几样 */
function fakeEl() {
  const classes = new Set(['unfold']);
  return {
    classes,
    attrs: /** @type {Record<string, string>} */ ({}),
    inert: false,
    classList: { toggle: (c, on) => { if (on) classes.add(c); else classes.delete(c); return on; } },
    setAttribute(k, v) { this.attrs[k] = v; },
  };
}

test('占着地方的一块：出来是 is-on、能点、读屏读；收回去去掉 is-on（CSS 放收的过渡），里面的东西不动，不能点、读屏不读', () => {
  const el = fakeEl();
  unfold(/** @type {any} */ (el), true);
  assert.ok(el.classes.has('is-on'));
  assert.equal(el.inert, false);
  assert.equal(el.attrs['aria-hidden'], 'false');
  unfold(/** @type {any} */ (el), false);
  assert.ok(!el.classes.has('is-on'));
  assert.equal(el.inert, true);
  assert.equal(el.attrs['aria-hidden'], 'true');
});

test('cubicBezier 照 CSS 的写法：两头是 0、1；ease 在一半时约 0.8024；linear 是直的', () => {
  const ease = cubicBezier(0.25, 0.1, 0.25, 1);
  assert.equal(ease(0), 0);
  assert.equal(ease(1), 1);
  assert.ok(Math.abs(ease(0.5) - 0.8024) < 0.001, String(ease(0.5)));
  const linear = cubicBezier(0, 0, 1, 1);
  for (const x of [0.1, 0.37, 0.9]) assert.ok(Math.abs(linear(x) - x) < 1e-6);
  assert.equal(ease(-1), 0, '越界的夹住');
  assert.equal(ease(2), 1);
});

test('parseBezier 读 CSS 里写的 cubic-bezier(…)；读不懂的照 ease', () => {
  assert.deepEqual(parseBezier(' cubic-bezier(0.25, 0.1, 0.25, 1)'), [0.25, 0.1, 0.25, 1]);
  assert.deepEqual(parseBezier('nonsense'), [0.25, 0.1, 0.25, 1]);
});

test('snapToPixels：高度对齐到整的屏幕像素（屏幕上一像素是 1 ÷ 放大倍数个 CSS 像素）', () => {
  const unit = 1 / 1.1;
  const h = snapToPixels(123.456, 1.1);
  assert.ok(Math.abs(h / unit - Math.round(h / unit)) < 1e-9, '是整的屏幕像素');
  assert.ok(Math.abs(h - 123.456) <= unit / 2 + 1e-9, '离原来的不到半个屏幕像素');
  assert.equal(snapToPixels(10, 1), 10);
});

test('追着滚（思考的预览）：临界阻尼的弹簧，从静止起步先慢后快、不越过目标、最后停在目标上；帧长不同走到的一样', async () => {
  const { spring } = await import('../../src/lib/motion.js');
  let s = { pos: 0, vel: 0 };
  const steps = [];
  for (let i = 0; i < 200 && (s.pos !== 100 || s.vel !== 0); i += 1) {
    const next = spring(s, 100, 16, 180);
    steps.push(next.pos - s.pos);
    assert.ok(next.pos <= 100, '不越过');
    s = next;
  }
  assert.deepEqual(s, { pos: 100, vel: 0 }, '停在目标上');
  assert.ok(steps[0] < steps[3], '起步慢：头一帧比后面几帧走得少');
  assert.ok(Math.max(...steps) < 100 * 16 / 180, '最快的一帧也比直线追的头一帧慢');
  let a = { pos: 0, vel: 0 };
  for (let i = 0; i < 6; i += 1) a = spring(a, 100, 30, 180);
  const b = spring(spring({ pos: 0, vel: 0 }, 100, 90, 180), 100, 90, 180);
  assert.ok(Math.abs(a.pos - b.pos) < 1e-9 && Math.abs(a.vel - b.vel) < 1e-9, '帧率不同，走到的地方一样');
  assert.deepEqual(spring({ pos: 0, vel: 0 }, 100, 16, 0), { pos: 100, vel: 0 }, '时间常数是 0（少动画）：直接到');
});
