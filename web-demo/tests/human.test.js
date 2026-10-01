// @ts-check
//! 给人看的字（蓝图 `web.md`「时间线的数」第 5 条）：握手以后照界面语言问核心的 `human.get`（核心施工 W-1），只留
//! 页面用得上的两格；问不到的交回一句话，由开机那里照工具名写。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { loadHuman } from '../src/core/human.js';

/** 记下问了什么、照给的回的假连接。 */
function fakeConn(reply) {
  const asked = [];
  return {
    asked,
    request(method, params) {
      asked.push([method, params]);
      return reply instanceof Error ? Promise.reject(reply) : Promise.resolve(reply);
    },
  };
}

test('照界面语言问核心的 human.get，留 tools 和 said；回应里别的格子不要', async () => {
  const conn = fakeConn({ language: 'ja', tools: { shell: { name: 'コマンド実行', subject: 'description' } }, said: { 'core/x': '{a} 件' } });
  const human = await loadHuman(/** @type {any} */ (conn), 'ja');
  assert.deepEqual(conn.asked, [['human.get', { language: 'ja' }]]);
  assert.deepEqual(human, { tools: { shell: { name: 'コマンド実行', subject: 'description' } }, said: { 'core/x': '{a} 件' } });
});

test('回应缺了格子的当空的；问不到（核心太旧、读不懂资源）照原样报出去', async () => {
  assert.deepEqual(await loadHuman(/** @type {any} */ (fakeConn({ language: 'zh' })), 'zh'), { tools: {}, said: {} });
  await assert.rejects(loadHuman(/** @type {any} */ (fakeConn(new Error('no such method'))), 'zh'), /no such method/);
});
