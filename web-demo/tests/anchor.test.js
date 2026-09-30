// @ts-check
//! 对话区跟着最新的时停在哪（蓝图 `web.md`「对话区」的滚动那一行）：内容变长往下走，变短不往回退、底下垫上。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { anchorAt, atBottom, pinAt } from '../src/model/anchor.js';

test('内容变长：停到内容底边，不垫', () => {
  assert.deepEqual(anchorAt(200, 500, 900), { top: 400, pad: 0 });
});

test('内容变短（一段收起）：停在到过的最深处，差的垫在底下', () => {
  assert.deepEqual(anchorAt(400, 500, 820), { top: 400, pad: 80 });
});

test('垫的被新的字填上：垫的跟着少，填满了不垫', () => {
  assert.deepEqual(anchorAt(400, 500, 860), { top: 400, pad: 40 });
  assert.deepEqual(anchorAt(400, 500, 950), { top: 450, pad: 0 });
});

test('内容还没一屏高：停在顶上，不垫', () => {
  assert.deepEqual(anchorAt(0, 500, 300), { top: 0, pad: 0 });
});

test('收得少（最新的底边还在视口四成往下）：照旧停在到过的最深处', () => {
  assert.deepEqual(anchorAt(400, 500, 820, 200), { top: 400, pad: 80 });
});

test('收得多、最新的内容要整个退到视口上面去（长的时间线收起）：视口跟着往上走，最新的底边留在视口四成的地方（2026-10-01）', () => {
  assert.deepEqual(anchorAt(400, 500, 300, 200), { top: 100, pad: 300 });
  assert.deepEqual(anchorAt(3000, 500, 1200, 200), { top: 1000, pad: 300 });
});

test('清空过的：跟着往上走也不越过「上下文已清空」那一行', () => {
  assert.deepEqual(anchorAt(400, 500, 300, 200, 350), { top: 350, pad: 550 });
});

test('离底边够近才算在底下', () => {
  assert.equal(atBottom({ scrollHeight: 1000, scrollTop: 460, clientHeight: 500 }, 48), true);
  assert.equal(atBottom({ scrollHeight: 1000, scrollTop: 400, clientHeight: 500 }, 48), false);
});

test('她的一段正文长过视口：第一行顶到视口最上面（留一点边）就停住', () => {
  // 正文从 1000 起、留 16px：视口到 984 就停
  assert.deepEqual(pinAt(900, 1000, 16), { top: 900, pinned: false }, '还没顶到');
  assert.deepEqual(pinAt(1200, 1000, 16), { top: 984, pinned: true }, '顶到了：停在正文开头上面一点');
  assert.deepEqual(pinAt(1200, null, 16), { top: 1200, pinned: false }, '没有在写的正文');
});
