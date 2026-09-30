// @ts-check
//! 编辑、写入点开以后的差异（蓝图 `tui.md`「编辑、写入点开的差异」，照 TUI 演示的 `diff.rs`）：照工具调用的参数比，按行排。
//!
//! 头不读核心的存储（`01-架构.md` 第五节第 1 条），所以只照参数：写入的全部是加上的，带行号；编辑的每一处按行比，
//! 不知道在文件的第几行，不带行号。TUI 用 `similar` 的 Myers 比法，这里用最长公共子序列：排出来的行可能差一点，
//! 加减的行数一样（都是最少的改法）。

/** 每处改动前后带几行不变的。 */
const CONTEXT = 3;

/**
 * @typedef {{mark: 'keep'|'removed'|'added'|'gap', number: number|null, text: string}} Line
 * @typedef {{lines: Line[], added: number, removed: number}} Diff
 */

/**
 * 比两份内容。新建的文件，`before` 传空的。一行连着它的换行比：最后一行有没有换行算不一样（照 `similar` 的
 * `from_lines`）。
 * @returns {Diff}
 */
export function compare(before, after) {
  const a = lines(before);
  const b = lines(after);
  const ops = walk(a, b);
  const changed = ops.map((op) => op.mark !== 'keep');
  // 离改动不超过三行的不变行留着，别的略过；隔开的两块之间一行略过
  const near = ops.map((_, i) => changed.slice(Math.max(0, i - CONTEXT), i + CONTEXT + 1).some(Boolean));
  const out = { lines: /** @type {Line[]} */ ([]), added: 0, removed: 0 };
  let last = -1;
  ops.forEach((op, i) => {
    if (!near[i]) return;
    if (last >= 0 && last !== i - 1) out.lines.push({ mark: 'gap', number: null, text: '' });
    last = i;
    if (op.mark === 'added') out.added += 1;
    if (op.mark === 'removed') out.removed += 1;
    out.lines.push({ mark: op.mark, number: op.number, text: op.text.replace(/\r?\n$/, '') });
  });
  return out;
}

/**
 * 照工具调用的参数排差异：有 `content` 的是写入，有 `edits` 的是编辑；都不是的是 `null`。
 * @returns {Diff|null}
 */
export function fromArgs(args) {
  if (typeof args?.content === 'string') return compare('', args.content);
  if (!Array.isArray(args?.edits)) return null;
  const out = { lines: /** @type {Line[]} */ ([]), added: 0, removed: 0 };
  args.edits.forEach((piece, i) => {
    if (i > 0) out.lines.push({ mark: 'gap', number: null, text: '' });
    const one = compare(String(piece?.old_string ?? ''), String(piece?.new_string ?? ''));
    out.added += one.added;
    out.removed += one.removed;
    // 片段里的行号不是文件里的行号，不写
    out.lines.push(...one.lines.map((l) => ({ ...l, number: null })));
  });
  return out;
}

/** 切成行，每行连着它的换行。 */
function lines(text) {
  return text.match(/[^\n]*\n|[^\n]+$/g) ?? [];
}

/**
 * 一行行对下来：不变的、删掉的、加上的。删和加挨着时先删后加。行号：删掉的是原来的，别的是改后的，从 1 数起。
 * @returns {{mark: 'keep'|'removed'|'added', number: number, text: string}[]}
 */
function walk(a, b) {
  const n = a.length;
  const m = b.length;
  // common[i][j]：a 从 i、b 从 j 往后的最长公共子序列有多长
  const common = Array.from({ length: n + 1 }, () => new Uint32Array(m + 1));
  for (let i = n - 1; i >= 0; i--) {
    for (let j = m - 1; j >= 0; j--) {
      common[i][j] = a[i] === b[j] ? common[i + 1][j + 1] + 1 : Math.max(common[i + 1][j], common[i][j + 1]);
    }
  }
  const out = [];
  let i = 0;
  let j = 0;
  while (i < n || j < m) {
    if (i < n && j < m && a[i] === b[j]) {
      out.push({ mark: /** @type {const} */ ('keep'), number: j + 1, text: b[j] });
      i += 1;
      j += 1;
    } else if (i < n && (j >= m || common[i + 1][j] >= common[i][j + 1])) {
      out.push({ mark: /** @type {const} */ ('removed'), number: i + 1, text: a[i] });
      i += 1;
    } else {
      out.push({ mark: /** @type {const} */ ('added'), number: j + 1, text: b[j] });
      j += 1;
    }
  }
  return out;
}
