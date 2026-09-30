// @ts-check
//! 吉祥物手里的东西（软件包 `mascot`，蓝图 `web.md`「吉祥物」第 5 条）：电脑、游戏机是清单里的像素图，几帧轮着放。每一帧一样大，
//! 用到的字都在图例里（图例照主题的吉祥物颜色取色，`.` 是空）。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const manifest = JSON.parse(readFileSync(new URL('../../packages/mascot/manifest.json', import.meta.url), 'utf8'));
const props = manifest.settings.props.default;
const colors = new Set(['head', 'head_shade', 'ear', 'ear_shade', 'fin', 'fin_shade', 'eye', 'outline', 'lamp']);

test('电脑、游戏机：每一帧一样大，用到的字都在图例里，图例只用吉祥物的颜色', () => {
  for (const name of ['laptop', 'console']) {
    const p = props[name];
    assert.ok(p.frames.length >= 2, `${name} 至少两帧才动得起来`);
    const [w, h] = [p.frames[0][0].length, p.frames[0].length];
    for (const f of p.frames) {
      assert.equal(f.length, h, `${name} 每一帧一样高`);
      for (const row of f) {
        assert.equal(row.length, w, `${name} 每一行一样宽：${row}`);
        for (const ch of row) assert.ok(ch === '.' || ch in props.legend, `${name} 的 ${ch} 在图例里`);
      }
    }
  }
  for (const v of Object.values(props.legend)) assert.ok(colors.has(/** @type {string} */ (v)), `${v} 是吉祥物的颜色`);
});
