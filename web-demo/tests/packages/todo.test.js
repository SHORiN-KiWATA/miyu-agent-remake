// @ts-check
//! 待办（蓝图 `web.md`「待办」，照 `tui.md`「后台命令、子代理和侧边栏」第 4 条和 TUI 演示 `ui/sidebar.rs` 的 `todo_lines`）：
//! 做完几项、收成几行。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { progress, allDone, fold } from '../../packages/todo/model.js';

/** 这个包的设置项的出厂值 */
const config = Object.fromEntries(Object.entries(JSON.parse(readFileSync(new URL('../../packages/todo/manifest.json', import.meta.url), 'utf8')).settings).map(([k, s]) => [k, s.default]));


/** 收成的几行写成字：记号照状态，收起的、还有的照种类。 */
const show = (rows) => rows.map((r) => (r.kind === 'item' ? `${r.todo.state}:${r.todo.text}` : `${r.kind}:${r.count}`));

/** `n` 项，前 `steps` 项做完、下一项在做、别的没做。 */
function todos(n, steps = 0) {
  return Array.from({ length: n }, (_, i) => ({ text: `第${i + 1}项`, state: i < steps ? 'done' : i === steps ? 'active' : 'pending' }));
}

test('做完几项、一共几项；全做完了才算做完，没有待办不算', () => {
  assert.deepEqual(progress(todos(3, 1)), { done: 1, total: 3 });
  assert.ok(!allDone(todos(3, 2)));
  assert.ok(allDone(todos(3, 3)));
  assert.ok(!allDone([]), '没有待办不算做完');
});

test('放得下的一项一行', () => {
  const rows = fold(todos(5, 2), config.rows, false);
  assert.deepEqual(show(rows), ['done:第1项', 'done:第2项', 'active:第3项', 'pending:第4项', 'pending:第5项']);
});

test('放不下：做完的收成一行，接着在做的和后面的，放不下的收成最后一行', () => {
  const rows = fold(todos(9, 3), 5, false);
  assert.deepEqual(show(rows), ['folded:3', 'active:第4项', 'pending:第5项', 'pending:第6项', 'more:3']);
  assert.equal(rows.length, 5, '最多 5 行');
});

test('放不下、还没做完一项：没有收起的那一行，最后一行是还有几项', () => {
  const rows = fold(todos(9), 5, false);
  assert.deepEqual(show(rows), ['active:第1项', 'pending:第2项', 'pending:第3项', 'pending:第4项', 'more:5']);
});

test('放不下、剩下的正好放得下：不写还有几项', () => {
  const rows = fold(todos(9, 5), 5, false);
  assert.deepEqual(show(rows), ['folded:5', 'active:第6项', 'pending:第7项', 'pending:第8项', 'pending:第9项']);
});

test('全做完了（停着让人看的那一会儿）：留最后一项看它打勾，前面的收成一行', () => {
  assert.deepEqual(show(fold(todos(9, 9), 5, false)), ['folded:8', 'done:第9项']);
  assert.deepEqual(show(fold(todos(3, 3), 5, false)), ['done:第1项', 'done:第2项', 'done:第3项']);
});

test('展开：全部列出', () => {
  assert.equal(fold(todos(9, 3), 5, true).length, 9);
  assert.deepEqual(fold([], 5, false), []);
});

test('出厂露 5 行（照 TUI），全做完了停 1.5 秒再收', () => {
  assert.equal(config.rows, 5);
  assert.equal(config.hold_ms, 1500);
});

test('核心的清单换成这一块的样子：pending 没做、in_progress 在做、completed 做完，不认识的当没做；空的是空的', async () => {
  const { fromCore } = await import('../../packages/todo/model.js');
  assert.deepEqual(fromCore([{ content: '读代码', status: 'completed' }, { content: '改', status: 'in_progress' }, { content: '测', status: 'pending' }, { content: '?', status: 'odd' }]),
    [{ text: '读代码', state: 'done' }, { text: '改', state: 'active' }, { text: '测', state: 'pending' }, { text: '?', state: 'pending' }]);
  assert.deepEqual(fromCore([]), []);
  assert.deepEqual(fromCore(undefined), []);
});
