// @ts-check
//! 换行（蓝图 `web.md`「按键」）：`Shift+Enter`，还有照 TUI 的 `Ctrl+J`。输入框、确认和提问抽屉里写字的框都用这一份。

/**
 * 这一下是不是换行：`Shift+Enter`、`Ctrl+J`（大写锁开着也算）；带了别的修饰键的、输入法在选字的不算。
 * @param {{key: string, ctrlKey: boolean, shiftKey: boolean, altKey: boolean, metaKey: boolean, isComposing?: boolean}} e
 */
export function isNewline(e) {
  if (e.isComposing || e.altKey || e.metaKey) return false;
  if (e.key === 'Enter') return e.shiftKey && !e.ctrlKey;
  return e.ctrlKey && !e.shiftKey && e.key.toLowerCase() === 'j';
}

/**
 * 在光标处换一行（选着字的换掉选着的），能撤销的照浏览器自己的撤销走；交给调用的一方接着量高度。
 * @param {HTMLTextAreaElement} box
 */
export function insertNewline(box) {
  // `insertText` 进浏览器的撤销记录；不支持的退回直接改值
  if (document.execCommand?.('insertText', false, '\n')) return;
  box.setRangeText('\n', box.selectionStart, box.selectionEnd, 'end');
  box.dispatchEvent(new Event('input', { bubbles: true }));
}
