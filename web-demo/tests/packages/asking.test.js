// @ts-check
//! 确认和提问的抽屉（蓝图 `web.md`「确认和提问」，照 `tui.md`「确认和提问的抽屉」）：按键怎么走、交出去的形状
//! （`question.answered`、`tool.approval_decided`）、确认的问题行、了结以后留下的几行。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { openAsk, openApproval, press, saveEdit, approvalHead, report } from '../../packages/asking/model.js';

const fake = JSON.parse(readFileSync(new URL('../../packages/asking/fake.json', import.meta.url), 'utf8'));
const ask = (id) => fake.asks.find((a) => a.body.call_id === id);
const approval = (id) => fake.approvals.find((a) => a.body.call_id === id);

/** 连着按几下，交回最后的抽屉和交出去的（有的话）。 */
function keys(d, ...list) {
  let out = { d, done: null, edit: null };
  for (const k of list) out = press(out.d, k);
  return out;
}

test('一道单选题：↑↓ 选，Enter 选了就直接交；交出去照 question.answered 的形状', () => {
  const d = openAsk(ask('demo_ask_1'));
  const r = keys(d, 'down', 'enter');
  assert.deepEqual(r.done, { kind: 'ask', answers: [{ picked: ['保留'] }] });
});

test('数字键等于移过去按 Enter；超出的不理', () => {
  assert.deepEqual(keys(openAsk(ask('demo_ask_1')), '2').done, { kind: 'ask', answers: [{ picked: ['保留'] }] });
  assert.equal(keys(openAsk(ask('demo_ask_1')), '9').done, null);
});

test('两道题：答了一道跳到下一道，都答了到「确认」页，Enter 交整份；多选的 Space 勾、Enter 交这一道', () => {
  const d = openAsk(ask('demo_ask_2'));
  let r = keys(d, 'space', 'down', 'space', 'enter');
  assert.equal(r.d.tab, 1);
  r = keys(r.d, 'down', 'enter');
  assert.equal(r.d.tab, 2, '到「确认」页');
  assert.equal(r.done, null);
  r = press(r.d, 'enter');
  assert.deepEqual(r.done, { kind: 'ask', answers: [{ picked: ['输入框', '时间线'] }, { picked: ['连伪终端走查一起跑'] }] });
});

test('多选一个没勾时 Enter 交光标所在的那一项；←→ 换题、到头停住', () => {
  const r = keys(openAsk(ask('demo_ask_2')), 'down', 'down', 'enter');
  assert.deepEqual([...r.d.checked[0]], [2]);
  assert.equal(keys(r.d, 'left', 'left', 'left').d.tab, 0);
  assert.equal(keys(r.d, 'right', 'right', 'right').d.tab, 2);
});

test('「输入其他答案」：Enter 进编辑，保存了算答了这一道（单选的清掉选的）；空的不算', () => {
  const d = openAsk(ask('demo_ask_1'));
  const r = keys(d, 'down', 'down', 'enter');
  assert.deepEqual(r.edit, 'other');
  assert.equal(saveEdit(r.d, '   ').done, null);
  assert.deepEqual(saveEdit(r.d, ' 先问问再说 ').done, { kind: 'ask', answers: [{ picked: [], text: '先问问再说' }] });
});

test('n 补一句话：带在这一道的回答里（notes），不跳题', () => {
  const r = keys(openAsk(ask('demo_ask_1')), 'n');
  assert.equal(r.edit, 'note');
  const saved = saveEdit(r.d, '缓存留着');
  assert.equal(saved.done, null);
  assert.deepEqual(press(saved.d, 'enter').done, { kind: 'ask', answers: [{ picked: ['删掉'], notes: '缓存留着' }] });
});

test('确认：提了放行规则的四项，没提的两项；允许交 decision，不允许进编辑写理由', () => {
  const full = openApproval(approval('demo_approve_1'));
  assert.deepEqual(full.questions[0].options.map((o) => o.decision), ['once', 'session', 'workspace', 'deny']);
  assert.deepEqual(keys(full, 'down', 'enter').done, { kind: 'approve', decision: 'session' });
  const bare = openApproval(approval('demo_approve_2'));
  assert.deepEqual(bare.questions[0].options.map((o) => o.decision), ['once', 'deny']);
  const r = keys(bare, 'down', 'enter');
  assert.equal(r.edit, 'reason');
  assert.deepEqual(saveEdit(r.d, '').done, { kind: 'approve', decision: 'deny' });
  assert.deepEqual(saveEdit(r.d, '别清').done, { kind: 'approve', decision: 'deny', reason: '别清' });
});

test('确认的问题行：写几个文件、家目录写 ~、工作区外的标出来；运行的写命令', () => {
  // 照 ctx.text：有的键写成「键+字段」，没有的交回键本身
  const has = new Set(['access.write', 'access.read', 'access.exec', 'access_other']);
  const text = (key, fields) => (has.has(key) ? `${key}${JSON.stringify(fields)}` : key);
  const head = approvalHead(approval('demo_approve_1').body, '/home/alice', text);
  assert.equal(head.title, 'access.write{"count":1,"tool":"write"}');
  assert.deepEqual(head.paths, [{ path: '~/.editorconfig', outside: true }]);
  assert.equal(approvalHead(approval('demo_approve_2').body, null, text).title, 'access.exec{"count":0,"tool":"cargo clean"}');
  assert.equal(approvalHead({ call_id: 'x', access: 'network', detail: { tool: 'web_fetch' } }, null, text).title, 'access_other{"count":0,"tool":"web_fetch"}');
});

test('了结以后留下的：提问一道一块、记下是谁问的，多选用「、」接、补充接在后面、没答的写未回答；确认允许的不留、不允许的带理由；取消的一行', () => {
  const d = openAsk(ask('demo_ask_2'));
  assert.deepEqual(report(d, { kind: 'ask', answers: [{ picked: ['输入框', '时间线'], notes: '先小改' }, { picked: [] }] }), {
    type: 'answered', who: null, rows: [{ label: '范围', answer: { text: '输入框、时间线', notes: '先小改' } }, { label: '测试', answer: null }],
  });
  const other = openAsk(ask('demo_ask_3'));
  assert.deepEqual(report(other, { kind: 'ask', answers: [{ picked: [], text: '都不用' }] }).rows, [{ label: other.questions[0].question, answer: { text: '都不用', notes: null } }]);
  assert.equal(report(other, { kind: 'ask', answers: [{ picked: [] }] }).who, '子代理 查资料', '子代理、后台命令问的记下是谁');
  const a = openApproval(approval('demo_approve_1'));
  assert.equal(report(a, { kind: 'approve', decision: 'once' }), null);
  assert.deepEqual(report(a, { kind: 'approve', decision: 'deny', reason: '别动' }), { type: 'denied', reason: '别动' });
  assert.deepEqual(report(a, { kind: 'approve', cancelled: true }), { type: 'cancelled', kind: 'approve' });
});
