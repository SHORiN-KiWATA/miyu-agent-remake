// @ts-check
//! 软件包 rail（右边的跳转条）：正在看哪一句；清单合法（出厂值过得了自己的校验）。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { reading } from '../../packages/rail/reading.js';
import { problems } from '../../src/kernel/config.js';

test('跳转条上正在看哪一句：顶边在视口上面三分之一以内的最后一句；一句都没到的算第一句', () => {
  // 视口 900 高：三分之一是 297
  assert.equal(reading([-800, -100, 200, 700], 900, 0.33), 2);
  assert.equal(reading([-800, -100, 400, 700], 900, 0.33), 1);
  assert.equal(reading([350, 700], 900, 0.33), 0);
  assert.equal(reading([], 900, 0.33), -1);
});

test('清单：出厂值过得了自己的校验', () => {
  const manifest = JSON.parse(readFileSync(new URL('../../packages/rail/manifest.json', import.meta.url), 'utf8'));
  assert.deepEqual(problems(manifest.settings), []);
});
