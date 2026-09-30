// @ts-check
//! 确认和提问的抽屉怎么走（蓝图 `web.md`「确认和提问」第 3–6 条，照 `tui.md`「确认和提问的抽屉」和 TUI 演示的 `drawer/`）：
//! 选到哪、勾了哪些、写了什么；按一下键交回新的抽屉，交了的交回照 `question.answered`、`tool.approval_decided` 的形状；
//! 确认的问题行；了结以后正文末尾留什么。纯函数，画在 `drawer.js`、`report.js`。
//!
//! 确认也当成一道题：选项是几种决定（`decision`），没有「输入其他答案」。

/**
 * @typedef {{label: string, description?: string, preview?: string, decision?: string}} Option
 * @typedef {{header?: string, question: string, options: Option[], multiple?: boolean}} Question
 * @typedef {{kind: 'ask'|'approve', id: string, who: string|null, body: any, questions: Question[], tab: number,
 *   cursor: number[], checked: Set<number>[], choice: (number|null)[], custom: string[], notes: string[], done: boolean[]}} Drawer
 *   `tab` 是第几道题，题数那一格是「确认」页（两道题及以上才有）；`choice` 单选选了哪一项；`custom` 自己写的；`notes` 补充的
 * @typedef {{kind: 'ask', answers: {picked: string[], text?: string, notes?: string}[]}|{kind: 'approve', decision: string, reason?: string}
 *   |{kind: 'ask'|'approve', cancelled: true}} Result
 * @typedef {{d: Drawer, done: Result|null, edit: 'other'|'note'|'reason'|null}} Step 按一下以后：新的抽屉、交出去的、要进编辑的
 */

/** 放行的四种决定，照先后；请求没提规则的只给头尾两种（核心会拒 `no_rule`）。 */
const DECISIONS = ['once', 'session', 'workspace', 'deny'];
const NEEDS_RULE = new Set(['session', 'workspace']);

/** @param {'ask'|'approve'} kind @param {{who?: string, body: any}} item @param {Question[]} questions @returns {Drawer} */
function make(kind, item, questions) {
  const n = questions.length;
  return {
    kind, id: item.body.call_id, who: item.who ?? null, body: item.body, questions, tab: 0,
    cursor: Array(n).fill(0), checked: questions.map(() => new Set()), choice: Array(n).fill(null),
    custom: Array(n).fill(''), notes: Array(n).fill(''), done: Array(n).fill(false),
  };
}

/** 一组题（`question.asked` 的 `body`，`who` 是谁在问）。 @param {{who?: string, body: {call_id: string, questions: Question[]}}} item */
export function openAsk(item) {
  return make('ask', item, item.body.questions);
}

/**
 * 一次确认（`tool.approval_requested` 的 `body`）：一道题，选项是几种决定，标题由画的一方照 `decision` 写。
 * @param {{who?: string, body: {call_id: string, access: string, rule?: any, detail?: any}}} item
 */
export function openApproval(item) {
  const options = DECISIONS.filter((x) => item.body.rule || !NEEDS_RULE.has(x)).map((decision) => ({ label: decision, decision }));
  return make('approve', item, [{ question: '', options }]);
}

/** 这一道能选的有几行：提问多一行「输入其他答案」。 @param {Drawer} d */
export function rows(d, q = d.tab) {
  return d.questions[q].options.length + (d.kind === 'ask' ? 1 : 0);
}

/** 有「确认」页（两道题及以上）。 @param {Drawer} d */
export function hasReview(d) {
  return d.kind === 'ask' && d.questions.length > 1;
}

/** 光标在不在「输入其他答案」上。 @param {Drawer} d */
export function onOther(d) {
  return d.kind === 'ask' && d.tab < d.questions.length && d.cursor[d.tab] === d.questions[d.tab].options.length;
}

/** @param {Drawer} d @param {Partial<Drawer>} patch @returns {Drawer} */
const next = (d, patch) => ({ ...d, ...patch });
/** @param {any[]} list @param {number} i @param {any} v */
const put = (list, i, v) => list.map((x, j) => (j === i ? v : x));

/**
 * 按一下键。`key`：`up` `down` `left` `right` `tab` `space` `enter` `n`，或者数字 `1`–`9`。
 * @param {Drawer} d
 * @param {string} key
 * @returns {Step}
 */
export function press(d, key) {
  const stay = { d, done: null, edit: null };
  const count = d.questions.length;
  const last = hasReview(d) ? count : count - 1;
  if (key === 'left') return { ...stay, d: next(d, { tab: Math.max(0, d.tab - 1) }) };
  if (key === 'right' || key === 'tab') return { ...stay, d: next(d, { tab: Math.min(last, d.tab + 1) }) };
  if (d.tab === count) return key === 'enter' ? { ...stay, done: submit(d) } : stay;
  const n = rows(d);
  const at = d.cursor[d.tab];
  if (key === 'up' || key === 'down') return { ...stay, d: next(d, { cursor: put(d.cursor, d.tab, (at + (key === 'down' ? 1 : n - 1)) % n) }) };
  if (key === 'n') return d.kind === 'ask' ? { ...stay, edit: 'note' } : stay;
  if (/^[1-9]$/.test(key)) {
    const k = Number(key) - 1;
    if (k >= n) return stay;
    const moved = next(d, { cursor: put(d.cursor, d.tab, k) });
    return d.questions[d.tab].multiple && k < d.questions[d.tab].options.length ? { ...stay, d: toggle(moved) } : press(moved, 'enter');
  }
  if (key === 'space') return d.questions[d.tab].multiple && !onOther(d) ? { ...stay, d: toggle(d) } : stay;
  if (key !== 'enter') return stay;
  if (onOther(d)) return { ...stay, edit: 'other' };
  if (d.kind === 'approve') {
    const decision = d.questions[0].options[at].decision ?? '';
    return decision === 'deny' ? { ...stay, edit: 'reason' } : { ...stay, done: { kind: 'approve', decision } };
  }
  const q = d.questions[d.tab];
  const answered = q.multiple
    ? next(d, { checked: d.checked[d.tab].size ? d.checked : put(d.checked, d.tab, new Set([at])) })
    : next(d, { choice: put(d.choice, d.tab, at), custom: put(d.custom, d.tab, '') });
  return advance(next(answered, { done: put(d.done, d.tab, true) }));
}

/** 能多选的：勾上、取消光标所在的那一项。 @param {Drawer} d */
function toggle(d) {
  const set = new Set(d.checked[d.tab]);
  const at = d.cursor[d.tab];
  if (set.has(at)) set.delete(at);
  else set.add(at);
  return next(d, { checked: put(d.checked, d.tab, set) });
}

/** 答了一道：只有一道题的直接交；别的跳到下一道没答的，都答了到「确认」页。 @param {Drawer} d @returns {Step} */
function advance(d) {
  if (!hasReview(d)) return { d, done: submit(d), edit: null };
  const count = d.questions.length;
  const after = [...Array(count).keys()].map((k) => (d.tab + 1 + k) % count).find((k) => !d.done[k]);
  return { d: next(d, { tab: after ?? count }), done: null, edit: null };
}

/**
 * 编辑完了保存：`other` 自己写的（空的不算答了；单选的清掉选的）、`note` 补充的（不跳题）、`reason` 不允许的理由（保存就交）。
 * @param {Drawer} d
 * @param {string} text
 * @param {'other'|'note'|'reason'} [what] 不给的照光标：在「输入其他答案」上是 `other`，确认是 `reason`
 * @returns {Step}
 */
export function saveEdit(d, text, what) {
  const kind = what ?? (d.kind === 'approve' ? 'reason' : onOther(d) ? 'other' : 'note');
  const value = text.trim();
  if (kind === 'reason') return { d, done: { kind: 'approve', decision: 'deny', ...(value ? { reason: value } : {}) }, edit: null };
  if (kind === 'note') return { d: next(d, { notes: put(d.notes, d.tab, value) }), done: null, edit: null };
  const withText = next(d, { custom: put(d.custom, d.tab, value) });
  if (!value) return { d: withText, done: null, edit: null };
  const q = d.questions[d.tab];
  return advance(next(withText, { choice: q.multiple ? d.choice : put(d.choice, d.tab, null), done: put(d.done, d.tab, true) }));
}

/** 交整份：照题目的先后一道一条。 @param {Drawer} d @returns {Result} */
function submit(d) {
  return {
    kind: 'ask',
    answers: d.questions.map((q, i) => {
      const picked = q.multiple
        ? [...d.checked[i]].sort((a, b) => a - b).map((k) => q.options[k].label)
        : d.choice[i] != null ? [q.options[/** @type {number} */ (d.choice[i])].label] : [];
      return { picked, ...(d.custom[i] ? { text: d.custom[i] } : {}), ...(d.notes[i] ? { notes: d.notes[i] } : {}) };
    }),
  };
}

/** 取消：提问算没回答，确认算跳过。 @param {Drawer} d @returns {Result} */
export function cancel(d) {
  return { kind: d.kind, cancelled: true };
}

/** 这一道现在的回答（「确认」页、标签上的 `✓` 用）：选的、自己写的，用「、」接；没答的是 `null`。 @param {Drawer} d @param {number} i */
export function answerOf(d, i) {
  const got = submit(d).answers[i];
  const parts = [...got.picked, ...(got.text ? [got.text] : [])];
  return parts.length ? parts.join('、') : null;
}

/**
 * 确认的问题行：`要写 1 个文件` 这类（`access.<种类>`，没有的 `access_other`），下面的路径（家目录写 `~`，工作区外的标出来）。
 * @param {{access: string, detail?: {tool?: string, paths?: {path: string, zone?: string}[]}}} body
 * @param {string|null} home
 * @param {(key: string, fields?: Record<string, any>) => string} text 照键取字（`ctx.text`：没有这个键的交回键本身）
 */
export function approvalHead(body, home, text) {
  const paths = body.detail?.paths ?? [];
  const access = body.access === 'execute' ? 'exec' : body.access;
  const fields = { count: paths.length, tool: body.detail?.tool ?? access };
  const key = `access.${access}`;
  const known = text(key, fields);
  return {
    title: known === key ? text('access_other', fields) : known,
    paths: paths.map((p) => ({ path: home && (p.path === home || p.path.startsWith(`${home}/`)) ? `~${p.path.slice(home.length)}` : p.path, outside: p.zone === 'outside' })),
  };
}

/**
 * 了结以后正文末尾留什么：提问一道一块、记下是谁问的（没有短名的写问题；没答的 `answer` 是 `null`）；确认允许的不留（`null`），不允许的带理由；
 * 取消的一行。
 * @param {Drawer} d
 * @param {Result} result
 */
export function report(d, result) {
  if ('cancelled' in result) return { type: 'cancelled', kind: d.kind };
  if (result.kind === 'approve') return result.decision === 'deny' ? { type: 'denied', reason: result.reason ?? null } : null;
  return {
    type: 'answered',
    who: d.who,
    rows: d.questions.map((q, i) => {
      const got = result.answers[i] ?? { picked: [] };
      const parts = [...got.picked, ...(got.text ? [got.text] : [])];
      return { label: q.header || q.question, answer: parts.length || got.notes ? { text: parts.join('、'), notes: got.notes ?? null } : null };
    }),
  };
}
