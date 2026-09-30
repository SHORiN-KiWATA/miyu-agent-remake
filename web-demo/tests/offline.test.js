// @ts-check
//! 连不上桥时写什么（蓝图 `web.md`「连核心」第 9 条）：口令用不了、地址里没带口令、桥没在跑、桥连不上核心，各写为什么、怎么办。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { loadRes } from './support.js';
import { offline } from '../src/model/offline.js';

loadRes();

test('口令用不了（桥重启过）、没带口令：说回起桥的终端开新链接', () => {
  const bad = offline('bad');
  assert.equal(bad.title, '连不上桥');
  assert.match(bad.why, /桥每次重启都换一个口令/);
  assert.match(bad.how, /#k=/);
  assert.match(offline('none').why, /没带口令/);
  assert.equal(offline('none').how, bad.how);
});

test('桥没在跑：说怎么起桥；桥连不上核心：照原话，叫看终端里的报错', () => {
  assert.match(offline('down').why, /桥没在跑/);
  assert.match(offline('down').how, /cargo run/);
  const core = offline('core', '找不到数据根：没有权限');
  assert.equal(core.why, '找不到数据根：没有权限');
  assert.match(core.how, /终端里的报错/);
});
