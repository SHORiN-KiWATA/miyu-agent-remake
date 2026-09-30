// @ts-check
//! 输入历史（蓝图 `web.md`「输入历史」）：记什么、`↑` `↓` 怎么翻（没在翻的时候光标在最前面才翻、翻着的时候第一段、最后一段）、
//! 两下 `Esc` 清掉的那句先拿回来、列表怎么搜、一条写成一行。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { remember, Recall, search, firstLine, commandHead, pieces } from '../src/model/history.js';

/** 光标在哪：最前面、第一段、最后一段 */
const START = { start: true, first: true, last: false };
const END = { start: false, first: true, last: true };
const MIDDLE = { start: false, first: false, last: false };

test('记：从新到旧，空的不记，和最新的一模一样的不再记一遍，最多记那么多条', () => {
  let items = [];
  items = remember(items, '第一句', 1, 3);
  items = remember(items, '   ', 2, 3);
  items = remember(items, '/theme', 3, 3);
  items = remember(items, '/theme', 4, 3);
  items = remember(items, '第三句', 5, 3);
  items = remember(items, '第四句', 6, 3);
  assert.deepEqual(items.map((x) => x.text), ['第四句', '第三句', '/theme']);
  assert.equal(items[0].at, 6);
});

const HISTORY = [{ text: '新', at: 3 }, { text: '中', at: 2 }, { text: '旧', at: 1 }];

test('↑ ↓：空着按 ↑ 翻出最新的；翻着的时候接着往更早的走，到头就停；↓ 往回走，走过最新的回到翻之前没发的那句', () => {
  const r = new Recall(HISTORY);
  assert.equal(r.older('', START), '新');
  assert.equal(r.older('新', END), '中', '翻着的、没改过：光标在末尾也翻');
  assert.equal(r.older('中', END), '旧');
  assert.equal(r.older('旧', END), '旧', '到头就停（接了这一下，不动）');
  assert.equal(r.newer('旧', END), '中');
  assert.equal(r.newer('中', END), '新');
  assert.equal(r.newer('新', END), '', '走过最新的：回到翻之前（空的）');
  assert.equal(r.newer('', END), null, '没在翻：↓ 不接');
});

test('没在翻：框里有字的，光标到最前面才翻，那句没发的记着，↓ 走回来还给它', () => {
  const r = new Recall(HISTORY);
  assert.equal(r.older('写了一半', END), null, '光标不在最前面：不翻（浏览器自己挪光标）');
  assert.equal(r.older('写了一半', START), '新');
  assert.equal(r.newer('新', END), '写了一半');
});

test('翻出来的改了就不算在翻：光标不在最前面不翻；到最前面再按，从最新的翻起，改过的那句当成没发的', () => {
  const r = new Recall(HISTORY);
  r.older('', START);
  r.older('新', END);
  assert.equal(r.older('中，改了', END), null);
  assert.equal(r.older('中，改了', START), '新');
  assert.equal(r.newer('新', END), '中，改了');
});

test('好几段的：翻着的时候光标不在第一段按 ↑、不在最后一段按 ↓，归浏览器挪光标', () => {
  const r = new Recall([{ text: '一\n二', at: 2 }, { text: '旧', at: 1 }]);
  assert.equal(r.older('', START), '一\n二');
  assert.equal(r.older('一\n二', MIDDLE), null);
  assert.equal(r.newer('一\n二', { start: false, first: true, last: false }), null);
  assert.equal(r.older('一\n二', { start: false, first: true, last: false }), '旧');
});

test('两下 Esc 清掉的那句：空着按 ↑ 先拿回它，再按才进历史；↓ 走回来还给它', () => {
  const r = new Recall(HISTORY);
  r.clear('清掉的话');
  assert.equal(r.older('', START), '清掉的话');
  assert.equal(r.older('清掉的话', END), '新', '拿回来的没改过：再按进历史');
  assert.equal(r.newer('新', END), '清掉的话');
  assert.equal(r.older('', START), '新', '拿回过一次就没了');
});

test('没发过话：↑ 不接', () => {
  assert.equal(new Recall([]).older('', START), null);
});

test('发出去了：不在翻了，下一次 ↑ 从新记的那一条翻起', () => {
  const r = new Recall(HISTORY);
  r.older('', START);
  r.record('刚发的', 9, 100);
  assert.equal(r.older('', START), '刚发的');
});

test('列表搜：不分大小写，留下包含这些字的，从新到旧，标出对得上的那几个字；空的全留、不标', () => {
  const items = [{ text: 'Rust 的生命周期', at: 3 }, { text: '看看 rust 和 go', at: 2 }, { text: '别的', at: 1 }];
  const hits = search(items, 'RUST');
  assert.deepEqual(hits.map((h) => h.text), ['Rust 的生命周期', '看看 rust 和 go']);
  assert.deepEqual(hits[1].marks, [[3, 7]]);
  assert.equal(search(items, '').length, 3);
  assert.deepEqual(search(items, '').at(0)?.marks, []);
  assert.deepEqual(search(items, 'zzz'), []);
});

test('一条写成一行：只写第一行，后面还有几行；命令的名字那一截', () => {
  assert.deepEqual(firstLine('一行'), { line: '一行', more: 0 });
  assert.deepEqual(firstLine('第一行\n第二行\n第三行'), { line: '第一行', more: 2 });
  assert.equal(commandHead('/theme'), 6);
  assert.equal(commandHead('/compact 重点保留设计'), 8);
  assert.equal(commandHead('/home/me/a.txt'), 0, '路径不算命令');
  assert.equal(commandHead('普通的话'), 0);
});

test('一行切成几段：命令的名字那一截、对得上的字各自标出来，两样叠着的都标；只切露出来的那些字', () => {
  assert.deepEqual(pieces('/theme 换一下', [[3, 8]], 6), [
    { text: '/th', cmd: true, mark: false },
    { text: 'eme', cmd: true, mark: true },
    { text: ' 换', cmd: false, mark: true },
    { text: '一下', cmd: false, mark: false },
  ]);
  assert.deepEqual(pieces('普通的话', [], 0), [{ text: '普通的话', cmd: false, mark: false }]);
  assert.deepEqual(pieces('第一行', [[5, 7]], 0), [{ text: '第一行', cmd: false, mark: false }], '标在露出来的字外面的不管');
});

test('带附件的：连附件（核心存好的那一份、发在哪个会话）一起记；只有附件没有字的也记；字一样但带了附件的不算重复', () => {
  const photo = { session: 's1', parts: { attachments: [{ blob: 'sha256:aa', name: 'a.png', media_type: 'image/png', size: 3 }] } };
  let items = remember([], '看这张图', 1, 10, photo);
  items = remember(items, '看这张图', 2, 10);
  items = remember(items, '', 3, 10, photo);
  items = remember(items, '', 4, 10);
  assert.deepEqual(items.map((x) => [x.text, x.session ?? null]), [['', 's1'], ['看这张图', null], ['看这张图', 's1']]);
  assert.equal(items[2].parts?.attachments[0].name, 'a.png');
});

test('翻着的时候交得出现在是哪一条（附件跟着它回来）；回到没发的那句、拿回清掉的那句时是 null', () => {
  const r = new Recall([{ text: '新', at: 2, session: 's1', parts: { attachments: [] } }, { text: '旧', at: 1 }]);
  r.clear('清掉的');
  r.older('', START);
  assert.equal(r.item(), null, '拿回清掉的那句');
  r.older('清掉的', END);
  assert.equal(r.item()?.session, 's1');
  r.older('新', END);
  assert.equal(r.item()?.text, '旧');
  r.newer('旧', END);
  r.newer('新', END);
  assert.equal(r.item(), null, '回到没发的那句');
});
