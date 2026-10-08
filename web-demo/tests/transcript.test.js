// @ts-check
//! 事件 → 正文的条目：你的话、她的回答、时间线的一段、收尾那一行（时间线的细节在 `timeline.test.js`）。规矩照 `tui.md`「正文」第 4、5、7 条。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { loadRes, sampleLog, ev, ms } from './support.js';
import { project } from '../src/model/transcript.js';

loadRes();

/** 条目写成一行字，好比对。 */
const brief = (items) => items.map((it) => `${it.type}: ${it.type === 'steps' ? it.steps.map((s) => s.name ?? s.kind).join(',') : it.slot ? `[${it.slot}] ${it.event.kind}` : it.text}`);

test('一个会话的条目：换了工作区一行；撤销掉的那一轮不见了；撤回的话不见了；收尾那一行照 TUI 写；压缩那一行照先后在；确认和提问了结的夹在问的那一步后面', () => {
  assert.deepEqual(brief(project(sampleLog()).items), [
    'note: 工作区：~/src/miyu',
    'note: 上下文已压缩',
    'user: 再看看 tests 目录',
    'done: ⏸ 已中断',
    'user: 把仓库里的 .editorconfig 装到我的家目录',
    'reply: 我把它复制过去。',
    'steps: shell',
    'note: [asking] tool.approval_decided',
    'reply: 好，不动家目录里那份。要对比两份的差别，跟我说一声。',
    'done: ▣  07:33 · deepseek/deepseek-v4 · 24.1s · 4.2k(C92%)',
    'user: 把旧的构建产物清一清',
    'reply: 清之前先问你一句。',
    'steps: ask_user',
    'note: [asking] question.answered',
    'reply: 好，build 目录保留，缓存我也先不动。',
    'done: ▣  07:40 · deepseek/deepseek-v4 · 19.2s · 4.5k(C97%)',
  ]);
});

test('收尾那一行的图标是这一轮开始时的级别：56 号那一轮开始时是只读', () => {
  const done = project(sampleLog()).items.filter((it) => it.type === 'done');
  assert.deepEqual(done.map((it) => it.level), ['read_only', 'workspace', 'workspace']);
});

test('撤销以后又恢复的，那一轮回来', () => {
  const reverted = brief(project(sampleLog(85)).items);
  assert.ok(!reverted.includes('user: 把旧的构建产物清一清'));
  const back = brief(project(sampleLog(86)).items);
  assert.ok(back.includes('user: 把旧的构建产物清一清'));
  assert.ok(back.includes('done: ▣  07:40 · deepseek/deepseek-v4 · 19.2s · 4.5k(C97%)'));
});

test('她的条目记着是哪一轮的：同一轮的挂在一个头像下面', () => {
  const items = project(sampleLog()).items;
  assert.deepEqual(items.filter((it) => it.type !== 'user' && it.type !== 'note').map((it) => it.turn), [56, 65, 65, 65, 65, 76, 76, 76, 76]);
});

test('在跑的一轮：还没收尾；在收的字接在后面，收完一块才算完', () => {
  const log = sampleLog(66);
  const live = { turn: 65, seen: 66, blocks: [{ kind: 'text', text: '我把它', done: false }] };
  const v = project(log, live);
  assert.deepEqual(v.running, { turn: 65, start: Date.parse('2026-09-25T07:33:00.000Z'), level: 'workspace' });
  assert.deepEqual(brief(v.items).slice(-2), ['user: 把仓库里的 .editorconfig 装到我的家目录', 'reply: 我把它']);
  assert.equal(v.items.at(-1).streaming, true);
});

test('出错结束的一轮：红的一行；供应商的原话照写，429 这几种前面加一句人话（以前的日志没有状态码，限速的当 429）', () => {
  const log = sampleLog(66);
  log.push(
    { seq: 67, at: '2026-09-25T07:33:02.000Z', kind: 'model.called', turn: 65, by: { kind: 'kernel' }, body: { seen: 66, messages: 3, result: 'error', error: { class: 'rate_limited', message: 'slow down' } } },
    { seq: 68, at: '2026-09-25T07:33:02.100Z', kind: 'turn.ended', turn: 65, by: { kind: 'kernel' }, body: { reason: 'error' } },
  );
  const done = project(log).items.at(-1);
  assert.equal(done.text, '出错了：被限速了，或者额度不够，过一会儿再试：slow down');
  assert.equal(done.tone, 'error');
});

// 排队的消息（蓝图 `web.md`「排队的消息」，照 `tui.md`「运行状态行和排队的消息」第 5 条）

/** 一轮在跑：你先说一句，她想了一下、调了一次工具；这时你又说了一句（第 6 条，带着这一轮）。 */
function queuedLog() {
  return [
    ev(1, 0, 'session.created', undefined, { permission: { level: 'workspace', read_only: false } }),
    ev(2, 0, 'message.user', undefined, { blocks: [{ type: 'text', text: '整理一下' }] }, { kind: 'person' }),
    ev(3, 0, 'turn.started', 3, { trigger: 2 }),
    ev(4, 2, 'message.assistant', 3, { seen: 3, blocks: [
      { type: 'reasoning', text: '先看看' },
      { type: 'tool_call', call_id: 'c1', name: 'shell', args: '{"command":"ls"}' },
    ] }),
    ev(5, 3, 'tool.result', 3, { call_id: 'c1', status: 'ok', blocks: [{ type: 'text', text: 'a' }] }),
    ev(6, 4, 'message.user', 3, { blocks: [{ type: 'text', text: '顺便看看 b' }] }, { kind: 'person' }),
  ];
}

test('回答时发的话先排着：不进正文，列在排着的里', () => {
  const view = project(queuedLog());
  assert.deepEqual(view.items.map((it) => it.type), ['user', 'steps']);
  assert.deepEqual(view.queued.map((q) => q.text), ['顺便看看 b']);
});

test('后来的一次请求听到了：挪到正文末尾，前面那段收起，接着的步另起一段', () => {
  const events = [...queuedLog(),
    ev(7, 6, 'message.assistant', 3, { seen: 6, blocks: [{ type: 'reasoning', text: '好' }, { type: 'text', text: '都看了。' }] })];
  const view = project(events);
  assert.deepEqual(view.items.map((it) => it.type), ['user', 'steps', 'user', 'steps', 'reply']);
  assert.equal(view.items[1].finished, true);
  assert.equal(view.items[2].text, '顺便看看 b');
  assert.deepEqual(view.queued, []);
});

test('在收的回复听到了（它的 seen 够着）：一样挪进正文', () => {
  const live = { turn: 3, seen: 6, blocks: [{ kind: 'reasoning', text: '好', done: false, start: ms(6), end: null }] };
  const view = project(queuedLog(), live);
  assert.deepEqual(view.items.map((it) => it.type), ['user', 'steps', 'user', 'steps']);
  assert.deepEqual(view.queued, []);
});

test('这一轮没听到、接着开了下一轮：它是下一轮的开头；编辑只在开这一轮的那一句上', () => {
  const events = [...queuedLog(),
    ev(7, 5, 'turn.ended', 3, { reason: 'interrupted' }),
    ev(8, 5, 'turn.started', 8, { trigger: 6 })];
  const view = project(events);
  assert.deepEqual(view.items.map((it) => `${it.type}:${it.turn ?? ''}`), ['user:3', 'steps:3', 'done:3', 'user:8', 'waiting:8']);
  assert.equal(view.items[3].opens, true, '开下一轮的那一句');
  assert.equal(view.items[0].opens, true, '开第一轮的那一句');
  assert.deepEqual(view.queued, []);
});

// 压缩、清空（蓝图 `web.md`「压缩、清空」）：人要的压缩、清空单开的那一轮没有 `trigger`，不画她的头（照 `tui.md`「正文」第 9 条）。

/** 先正常说一轮，接着单开一轮没有 `trigger` 的（手动压缩、清空）：这一轮还没出字。 */
function manualLog() {
  return [
    ev(1, 0, 'session.created', undefined, { permission: { level: 'workspace', read_only: false } }),
    ev(2, 0, 'message.user', undefined, { blocks: [{ type: 'text', text: '整理一下' }] }, { kind: 'person' }),
    ev(3, 0, 'turn.started', 3, { trigger: 2 }),
    ev(4, 2, 'message.assistant', 3, { seen: 3, blocks: [{ type: 'text', text: '好。' }] }),
    ev(5, 3, 'turn.ended', 3, { reason: 'completed' }),
    ev(6, 4, 'turn.started', 6, {}),
  ];
}

test('人要的压缩、清空单开的那一轮（没有 trigger）在跑、还没出字：正文里没有三个球', () => {
  assert.deepEqual(project(manualLog()).items.map((it) => it.type), ['user', 'reply', 'done']);
});

test('普通一轮在跑、还没出字：正文里照旧有三个球', () => {
  const view = project(manualLog().slice(0, 3));
  assert.equal(view.items.at(-1).type, 'waiting');
  assert.equal(view.items.at(-1).turn, 3);
});

test('被退回的：从排着的里去掉，不画', () => {
  const events = [...queuedLog(), ev(7, 5, 'message.withdrawn', 3, { messages: [6] })];
  assert.deepEqual(project(events).queued, []);
  assert.ok(!project(events).items.some((it) => it.text === '顺便看看 b'));
});

test('没在回答了还排着的（没接着开）：进正文，不一直挂着', () => {
  const events = [...queuedLog(), ev(7, 5, 'turn.ended', 3, { reason: 'completed' })];
  const view = project(events);
  assert.deepEqual(view.queued, []);
  assert.equal(view.items.at(-1).text, '顺便看看 b');
});

test('这一轮结束和接着开下一轮之间先来了别的事（起了标题）：排着的照旧是下一轮的开头，不提前进正文', () => {
  const events = [...queuedLog(),
    ev(7, 5, 'turn.ended', 3, { reason: 'interrupted' }),
    ev(8, 5, 'session.meta_changed', undefined, { title: '看文件' }),
    ev(9, 5, 'turn.started', 9, { trigger: 6 })];
  const view = project(events);
  assert.deepEqual(view.items.map((it) => `${it.type}:${it.turn ?? ''}`), ['user:3', 'steps:3', 'done:3', 'user:9', 'waiting:9']);
  assert.equal(view.items[3].opens, true, '开下一轮的那一句');
});

test('/stop 留着的排着的话（O-6 的 keep）：这一轮结束以后来了别的事（命令记录、清空），照日志的先后进正文，不压到后来的那几行下面', () => {
  const events = [...queuedLog(),
    ev(7, 5, 'turn.ended', 3, { reason: 'interrupted' }),
    ev(8, 5, 'command.ran', undefined, { text: '/stop', command: 'stop' }, { kind: 'person', account: 'admin' }),
    ev(9, 6, 'turn.started', 9, {}),
    ev(10, 6, 'context.compacted', 9, { upto: 8, summary: '', trigger: 'clear' }),
    ev(11, 6, 'turn.ended', 9, { reason: 'completed' })];
  const view = project(events);
  const at = view.items.findIndex((it) => it.type === 'user' && it.text === '顺便看看 b');
  const clear = view.items.findLastIndex((it) => it.type !== 'user');
  assert.ok(at >= 0 && at < clear, `排着的那句在清空那一行前面：${view.items.map((it) => it.type).join(',')}`);
  assert.deepEqual(view.queued, []);
});

test('你的话里的附件：图片、文件照先后，字照旧；只有附件的字是空的（蓝图「附件」第 6 条）', () => {
  const img = { type: 'image', blob: `sha256:${'a'.repeat(64)}`, media_type: 'image/png', width: 800, height: 600 };
  const file = { type: 'file', blob: `sha256:${'b'.repeat(64)}`, name: '报告.pdf', media_type: 'application/pdf' };
  const view = project([
    ev(1, 0, 'session.created', undefined, { permission: { level: 'workspace', read_only: false } }),
    ev(2, 0, 'message.user', undefined, { blocks: [{ type: 'text', text: '看看这两个' }, img, file] }, { kind: 'person' }),
    ev(3, 1, 'message.user', undefined, { blocks: [img] }, { kind: 'person' }),
  ]);
  const [first, second] = view.items;
  assert.equal(first.text, '看看这两个');
  assert.deepEqual(first.attachments, [
    { kind: 'image', blob: img.blob, media_type: 'image/png', width: 800, height: 600, name: null },
    { kind: 'file', blob: file.blob, media_type: 'application/pdf', width: null, height: null, name: '报告.pdf' },
  ]);
  assert.equal(second.text, '');
  assert.equal(second.attachments.length, 1);
  assert.deepEqual(project([ev(1, 0, 'message.user', undefined, { blocks: [{ type: 'text', text: '只有字' }] }, { kind: 'person' })]).items[0].attachments, []);
});

test('图片块带了名字的（核心 3-9 四补）照带着，灯箱的说明用它', () => {
  const img = { type: 'image', blob: `sha256:${'a'.repeat(64)}`, media_type: 'image/png', width: 8, height: 6, name: '晚霞.png' };
  const [it] = project([ev(1, 0, 'message.user', undefined, { blocks: [img] }, { kind: 'person' })]).items;
  assert.equal(it.attachments[0].name, '晚霞.png');
});

test('收尾那一行的别的原因：前面权限级别的图标，后面界面语言的字；不认识的照原样（2026-10-01）', () => {
  const ended = (reason) => project([...queuedLog(), ev(7, 5, 'turn.ended', 3, { reason })]).items.find((it) => it.type === 'done')?.text;
  assert.equal(ended('restarted'), '▣  已重启');
  assert.equal(ended('aborted'), '▣  核心上次在这一轮崩了，没做完');
  assert.equal(ended('step_limit'), '▣  请求次数到了上限，停了');
  assert.equal(ended('mystery'), 'mystery');
});

test('确认和提问了结以后留的：在了结的那一条处放一条交给软件包画（slot asking，带着问的那一条），夹在她这一轮里：问的那一步后面、她接着说的前面；允许了的不放、打断的放', async () => {
  const { group } = await import('../src/model/group.js');
  const at = (n) => `2026-10-07T00:00:${String(n).padStart(2, '0')}.000Z`;
  const ev = (seq, kind, body, more = {}) => ({ seq, at: at(seq), kind, turn: 2, by: { kind: 'kernel' }, body, ...more });
  const events = [
    { seq: 1, at: at(1), kind: 'message.user', by: { kind: 'person' }, body: { blocks: [{ type: 'text', text: '问我' }] } },
    ev(2, 'turn.started', { trigger: 1 }),
    ev(3, 'message.assistant', { blocks: [{ type: 'tool_call', call_id: 'c1', name: 'ask_user', args: '{}' }], seen: 2 }),
    ev(4, 'question.asked', { call_id: 'c1', questions: [{ question: '要吗？', options: [{ label: '要' }] }] }),
    ev(5, 'question.answered', { call_id: 'c1', answers: [{ picked: ['要'] }] }),
    ev(6, 'tool.result', { call_id: 'c1', status: 'ok', blocks: [] }),
    ev(7, 'message.assistant', { blocks: [{ type: 'tool_call', call_id: 'c2', name: 'write', args: '{}' }], seen: 6 }),
    ev(8, 'tool.approval_requested', { call_id: 'c2', access: 'write' }),
    ev(9, 'tool.approval_decided', { call_id: 'c2', decision: 'once' }),
    ev(10, 'tool.result', { call_id: 'c2', status: 'ok', blocks: [] }),
    ev(11, 'message.assistant', { blocks: [{ type: 'text', text: '好的，收到。' }], seen: 10 }),
    ev(12, 'turn.ended', { reason: 'done' }),
  ];
  const { items } = project(events, null);
  const kinds = items.map((it) => (it.slot ? `slot:${it.event.kind}` : it.type));
  const slot = kinds.indexOf('slot:question.answered');
  assert.ok(slot > kinds.indexOf('steps') && slot < kinds.indexOf('reply'), `问的那一步后面、她接着说的前面：${kinds.join(' ')}`);
  assert.equal(kinds.filter((k) => k.startsWith('slot:')).length, 1, '允许了的不放');
  assert.equal(items[slot].asked.kind, 'question.asked');
  const blocks = group(items);
  assert.equal(blocks.filter((b) => b.kind === 'her').length, 1, '夹在她这一块里，不另起一块');
  // 打断：问过、没答就有了结果
  const cut = [...events.slice(0, 4), ev(5, 'tool.result', { call_id: 'c1', status: 'cancelled', blocks: [] }), ev(6, 'turn.ended', { reason: 'interrupted' })];
  assert.deepEqual(project(cut, null).items.filter((it) => it.slot).map((it) => it.event.body.status), ['cancelled']);
});

test('按页读：派后台命令的那条还在没读的页里，回报那一行照种子里的任务名单写标题（核心 9-6 再补的 view.page jobs）', () => {
  const seed = { seq: 0, at: '2026-10-08T00:00:00Z', kind: 'jobs.seed', body: { jobs: [], known: [{ job: 'j1', what: 'command', title: '后台睡三秒' }] } };
  const reported = { seq: 40, at: '2026-10-08T00:00:03Z', kind: 'job.reported', body: { job: 'j1', reason: 'exited', exit_code: 0, duration_ms: 3100 } };
  const without = JSON.stringify(project([reported]).items);
  const withSeed = JSON.stringify(project([seed, reported]).items);
  assert.ok(!without.includes('后台睡三秒'), '没有名单时拿不到标题');
  assert.ok(withSeed.includes('后台睡三秒'));
});

test('换了工作区：一行「工作区：路径」，不属于哪一轮，路径照原样带着（家目录由画的那一头写成 ~）', () => {
  const log = [
    ev(1, 0, 'session.created', null, { cwd: '/home/u/a' }),
    ev(2, 5, 'session.workspace_changed', null, { cwd: '/home/u/b' }, { kind: 'account', account: 'admin' }),
  ];
  const notes = project(log).items.filter((it) => it.type === 'note');
  assert.equal(notes.length, 1);
  assert.equal(notes[0].text, '工作区：/home/u/b');
  assert.equal(notes[0].path, '/home/u/b');
  assert.equal(notes[0].turn, null);
});
