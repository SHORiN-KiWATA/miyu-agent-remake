// @ts-check
//! 正文里不是你说的、不挂在她头下的几样（蓝图 `web.md`「不是你说的话」，收尾那一行的出错写法）：谁说的、出错那一句。
//! 旁白（压缩、清空、后台任务、换了模型……）照条目画的见 `entries.test.js`。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { loadRes } from './support.js';
import { failureText, speakerOf } from '../src/model/notes.js';

loadRes();

const CHILD = '0199a000-0000-7000-8000-000000000002';
const PARENT = '0199a000-0000-7000-8000-0000000000ff';

test('谁说的：你的、别的账号的、子代理发给她的（标题照派它的那次）、派它的会话、别的会话、别的 harness、平台上的人', () => {
  const jobs = new Map([['j2', { what: 'agent', title: '查文档', session: CHILD, command: null }]]);
  const of = (by) => speakerOf(by, jobs, PARENT);
  assert.deepEqual(of({ kind: 'person', account: 'admin' }), { kind: 'person', account: 'admin', name: 'admin' });
  assert.deepEqual(of({ kind: 'person', account: 'alice' }), { kind: 'person', account: 'alice', name: 'alice' });
  assert.deepEqual(of({ kind: 'session', id: CHILD }), { kind: 'agent', account: null, name: '子代理 · 查文档' });
  assert.equal(of({ kind: 'session', id: PARENT }).kind, 'parent');
  assert.match(of({ kind: 'session', id: '0199a000-0000-7000-8000-00000000abcd' }).name, /0000abcd/);
  assert.deepEqual(of({ kind: 'harness', name: 'claude-code' }), { kind: 'harness', account: null, name: 'claude-code · 外部 agent' });
  assert.deepEqual(of({ kind: 'external', platform: 'qq' }), { kind: 'external', account: null, name: '外部' });
});

test('出错那一句：402、404 加人话；内核自己查出来的写分类，有原话的接后面；没原话的写分类', () => {
  assert.equal(failureText({ class: 'other', message: 'no money', status: 402 }), '额度不足：no money');
  assert.equal(failureText({ class: 'other', message: 'model not found', status: 404 }), '未找到，请检查端点地址和模型名：model not found');
  assert.equal(failureText({ class: 'bad_stream', message: 'eof' }), '响应流异常：eof');
  assert.equal(failureText({ class: 'empty_reply', message: '' }), '响应为空');
  assert.equal(failureText({ class: 'auth', message: '' }), '认证失败');
  assert.equal(failureText({ class: 'no_model', message: 'models.chat is not set' }), '没有可用的模型：models.chat is not set', '没有可用的模型（8-6）是内核查出来的');
  assert.equal(failureText({ class: 'cooling', message: 'all candidates cooling: dev/m key 1 rate_limited until 10:05' }), '模型都在冷却：all candidates cooling: dev/m key 1 rate_limited until 10:05', '模型都在冷却（8-9）没发出去，也是内核查出来的');
});
