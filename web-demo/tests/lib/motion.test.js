// @ts-check
//! 退场（蓝图 `web.md`「动效」）：一个节点的动画要走多久（照计算好的 `animation-duration`、`animation-delay`，几段取最长的）；
//! 在流里占着地方的一块出来、收回去（`unfold`）。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { span, unfold } from '../../src/lib/motion.js';

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
