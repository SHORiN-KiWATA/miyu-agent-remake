// @ts-check
//! 斜杠命令怎么认、怎么筛（蓝图 `web.md`「斜杠命令」「命令列表」，照 TUI 演示 `commands.rs`、`menu.rs` 的测试）：
//! 像不像命令名、带参数的、别名、先前缀后包含、`Esc` 关掉以后字变了才再开、撤销放回框里的那句。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { loadRes, sampleLog } from './support.js';
import { typed, menuTyped, filter, find, read, Menu, revertedSaid, revertedMessage, keptAttachments } from '../src/model/commands.js';

const res = loadRes();
const list = res.commands.commands;
const names = (specs) => specs.map((s) => s.name);

test('出厂的清单照蓝图：去掉 /icons、/exit，加 /clear、/stop（O-6，2026-10-07）、/recap、/language、/redo、/edit（/demo-todo 由软件包 todo 登记，/settings、/config、/pkg 由软件包 settings 登记）；/compact、/sessions（带着搜的词，2026-10-02）、/language、/model 带参数；真的几条各有各的做法（/model 2026-10-01 起是真的）', () => {
  assert.deepEqual(names(list), ['undo', 'restore', 'redo', 'edit', 'compact', 'clear', 'stop', 'recap', 'theme', 'new', 'sessions', 'language', 'model', 'readonly', 'level',
    'tools', 'copy', 'help']);
  assert.deepEqual(list.filter((s) => s.args).map((s) => s.name), ['compact', 'sessions', 'language', 'model']);
  const runs = Object.fromEntries(list.filter((s) => s.run !== 'fake').map((s) => [s.name, s.run]));
  assert.deepEqual(runs, { undo: 'revert', restore: 'unrevert', redo: 'redo', edit: 'edit', compact: 'compact', clear: 'clear', stop: 'stop', recap: 'recap', theme: 'theme', new: 'new', sessions: 'sessions', copy: 'copy',
    language: 'language', model: 'model' });
  assert.deepEqual(names(list.filter((s) => s.run === 'fake')), ['readonly', 'level', 'tools', 'help']);
  assert.deepEqual(find(list, 'resume')?.name, 'sessions', '2026-10-02：/sessions 加 /resume 别名');
});

test('像命令名的才算：英文字母打头，只有字母、数字、-、_；刚打一个 / 也算；路径、中文不算', () => {
  assert.equal(typed('/undo'), 'undo');
  assert.equal(typed('/'), '');
  assert.equal(typed('/home/me/a.txt'), null);
  assert.equal(typed('undo'), null);
  assert.equal(typed('/你吃了吗'), null);
  assert.equal(typed('/1st'), null);
  assert.equal(typed('/foo-bar_2'), 'foo-bar_2');
  assert.equal(typed('/foo 带参数'), 'foo');
  assert.equal(typed(''), null);
});

test('列表开不开：名字后面打了空格就收起（名字打全了）', () => {
  assert.equal(menuTyped('/comp'), 'comp');
  assert.equal(menuTyped('/'), '');
  assert.equal(menuTyped('/compact '), null);
  assert.equal(menuTyped('/compact 重点'), null);
  assert.equal(menuTyped('/你吃了吗'), null);
  assert.equal(menuTyped('/ compact'), null, '名字要紧挨着 /');
  assert.equal(menuTyped('/undo\n'), null, '换行也是空白');
});

test('回车时这一行是什么：命令带着参数；没有这个命令的弹「命令不存在」；别的照普通的话发', () => {
  const r = read(list, '/compact 重点保留 数据库设计');
  assert.equal(r.kind, 'command');
  assert.equal(r.spec?.name, 'compact');
  assert.equal(r.words, '重点保留 数据库设计');
  assert.deepEqual([read(list, '/compact').kind, read(list, '/compact').words], ['command', null]);
  assert.equal(read(list, '/compact   ').words, null, '只有空白：没写');
  assert.equal(read(list, '/undo').spec?.name, 'undo');
  assert.equal(read(list, '/rewind').spec?.name, 'undo', '别名一样执行');
  assert.equal(read(list, '/resume').spec?.name, 'sessions', '别名一样执行（2026-10-02）');
  assert.equal(read(list, '/theme 深色').kind, 'talk', '不带参数的命令后面跟了字：一句话');
  assert.equal(read(list, '/etc 目录是干什么的').kind, 'talk', '不弹命令不存在');
  assert.equal(read(list, '/nosuch').kind, 'unknown');
  assert.equal(read(list, '/').kind, 'unknown');
  assert.equal(read(list, '你好').kind, 'talk');
  assert.equal(read(list, '/你吃了吗').kind, 'talk');
  assert.equal(read(list, '/home/me/a.txt').kind, 'talk');
  assert.equal(read(list, '/ compact').kind, 'talk', '名字要紧挨着 /');
});

test('边打边筛：名字（或别名）开头的排前面，含着的排后面，各照清单的先后；不分大小写', () => {
  assert.deepEqual(names(filter(list, 're')), ['undo', 'restore', 'redo', 'clear', 'recap', 'sessions', 'readonly'], 'rewind、reset（别名）、restore、redo、recap、resume（别名）、readonly 开头');
  assert.deepEqual(names(filter(list, 'e')), ['edit', 'undo', 'restore', 'redo', 'clear', 'recap', 'theme', 'new', 'sessions', 'language', 'model', 'readonly', 'level',
    'help'], 'e 开头的只有 /edit，排前面；别的都是含着的，照清单的先后');
  assert.deepEqual(names(filter(list, 'rew')), ['undo'], '筛的时候别名也算，列表里写正名');
  assert.deepEqual(names(filter(list, 'T')), ['theme', 'tools', 'restore', 'edit', 'compact', 'clear', 'stop']);
  assert.equal(filter(list, '').length, list.length, '刚打一个 /：全部');
  assert.deepEqual(filter(list, 'zzz'), []);
});

test('名字或别名正好对上的那一条；/redo 是重做（原来恢复撤销的旧名字，现在照 TUI 是 session.redo）', () => {
  assert.equal(find(list, 'restore')?.run, 'unrevert');
  assert.equal(find(list, 'rewind')?.run, 'revert');
  assert.equal(find(list, 'redo')?.run, 'redo');
  assert.equal(find(list, 'resume')?.name, 'sessions', '2026-10-02：/resume 别名');
  assert.equal(find(list, ''), null);
});

test('列表：字变了选中回到第一条；上下到头就停', () => {
  const m = new Menu();
  assert.equal(m.sync('/', list).length, list.length);
  m.step(true, list.length);
  m.step(true, list.length);
  assert.equal(m.selected, 2);
  m.step(false, list.length);
  m.step(false, list.length);
  m.step(false, list.length);
  assert.equal(m.selected, 0, '到头就停');
  for (let i = 0; i < 30; i++) m.step(true, list.length);
  assert.equal(m.selected, list.length - 1);
  m.sync('/t', list);
  assert.equal(m.selected, 0, '字变了回到第一条');
});

test('列表：Esc 关掉以后，字变了才再打开；删掉 / 再打一个照常打开', () => {
  const m = new Menu();
  assert.ok(m.sync('/u', list).length > 0);
  m.dismiss('/u');
  assert.deepEqual(m.sync('/u', list), []);
  assert.ok(m.sync('/un', list).length > 0);
  m.dismiss('/un');
  assert.deepEqual(m.sync('', list), []);
  assert.ok(m.sync('/', list).length > 0, '不是命令了再打 /：照常打开');
  assert.deepEqual(m.sync('/compact ', list), [], '打了空格收起');
  assert.deepEqual(m.sync('/zzz', list), [], '一条都没有不开');
});

test('撤销成了：撤掉的那一轮里你说的话（整段），照 turn.reverted 找回；找不到的是空的', () => {
  const events = sampleLog(86);
  assert.equal(revertedSaid(events, 53), '看看 src 目录');
  assert.equal(revertedSaid(events, 85), '把旧的构建产物清一清');
  assert.equal(revertedSaid(events, 86), null, '恢复那一条不是撤销');
  assert.equal(revertedSaid(events, 999), null, '还没收到那一条');
  assert.equal(revertedSaid(events, undefined), null);
  const multi = [
    { seq: 1, kind: 'message.user', by: { kind: 'person' }, body: { blocks: [{ type: 'text', text: '第一行\n第二行' }] } },
    { seq: 2, kind: 'turn.started', turn: 2, body: { trigger: 1 } },
    { seq: 3, kind: 'turn.reverted', body: { turns: [2] } },
  ];
  assert.equal(revertedSaid(multi, 3), '第一行\n第二行', '好几行的整段放回');
  const kernel = [{ seq: 1, kind: 'turn.started', turn: 1, body: {} }, { seq: 2, kind: 'turn.reverted', body: { turns: [1] } }];
  assert.equal(revertedSaid(kernel, 2), null, '不是人开的那一轮');
});

test('撤掉的那一句带的附件回到框里：照图片、文件块换成附件包的样子，大小照输入历史，查不到是 null，没名字的图片起一个', () => {
  const said = { seq: 41, kind: 'message.user', by: { kind: 'person' }, body: { blocks: [
    { type: 'text', text: '看看这两张' },
    { type: 'image', blob: 'b1', media_type: 'image/png', width: 10, height: 10 },
    { type: 'file', blob: 'b2', media_type: 'application/pdf', name: '报告.pdf' },
  ] } };
  const started = { seq: 42, kind: 'turn.started', turn: 42, body: { trigger: 41 } };
  const reverted = { seq: 50, kind: 'turn.reverted', body: { turns: [42] } };
  assert.equal(revertedMessage([said, started, reverted], 50), said);
  assert.equal(revertedMessage([said, started], 50), null, '撤销那一条还没收到');
  const history = [{ sent: '看看这两张', parts: { attachments: [{ blob: 'b2', name: '报告.pdf', media_type: 'application/pdf', size: 2048 }] } }];
  assert.deepEqual(keptAttachments(said, history), [
    { blob: 'b1', name: 'image.png', media_type: 'image/png', size: null },
    { blob: 'b2', name: '报告.pdf', media_type: 'application/pdf', size: 2048 },
  ]);
  assert.deepEqual(keptAttachments({ body: { blocks: [{ type: 'image', blob: 'b3', media_type: 'image/svg+xml' }] } }, []), [{ blob: 'b3', name: 'image.svg', media_type: 'image/svg+xml', size: null }]);
});
