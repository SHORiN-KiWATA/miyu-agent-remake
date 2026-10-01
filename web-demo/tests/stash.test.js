// @ts-check
//! Ctrl+S 暂存（蓝图 `web.md`「按键」，照 `tui.md`）：框里有字存起来、清空；空着取回来；两边都有互换；两边都空什么都不做。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { stash } from '../src/model/stash.js';

const none = { text: '', blocks: [] };
const draft = { text: '看看 [a.rs]', blocks: [['[a.rs]', 'src/a.rs']] };
const other = { text: '另一句', blocks: [] };

test('有字：存起来、清空；空着：取回来；两边都有：互换；都空：没有暂存的', () => {
  assert.deepEqual(stash(draft, null), { input: none, stashed: draft, said: 'stashed' });
  assert.deepEqual(stash(none, draft), { input: draft, stashed: null, said: 'restored' });
  assert.deepEqual(stash(other, draft), { input: draft, stashed: other, said: 'swapped' });
  assert.deepEqual(stash(none, null), { input: none, stashed: null, said: 'empty' });
  assert.deepEqual(stash({ text: '   \n', blocks: [] }, null), { input: { text: '   \n', blocks: [] }, stashed: null, said: 'empty' }, '只有空白的不算有字');
});
