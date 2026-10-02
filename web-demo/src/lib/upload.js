// @ts-check
//! 分块上传一个附件（蓝图 `web.md`「附件」第 2 条，核心施工 W-5）：`blob.open` 拿编号，`blob.write` 一块一块写（base64），
//! `blob.close` 存好，回应和 `blob.put` 一样。`offset` 接不上（`upload_offset`）、存的时候没收齐（`upload_incomplete`）的，照拒绝里的
//! `data.received` 从那里接着传；一直接不上（连着几次没往前走）的不再来，照原样抛。读字节由宿主给（`files.read`），这里不碰平台；
//! 一块多大、接几次由用的人给（软件包 `attachments` 的设置项）。

const ALPHABET = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';

/** 字节写成 base64（标准字母表，补 `=`）。 @param {Uint8Array} bytes */
export function toBase64(bytes) {
  let out = '';
  for (let i = 0; i < bytes.length; i += 3) {
    const n = (bytes[i] << 16) | ((bytes[i + 1] ?? 0) << 8) | (bytes[i + 2] ?? 0);
    out += ALPHABET[(n >> 18) & 63] + ALPHABET[(n >> 12) & 63];
    out += i + 1 < bytes.length ? ALPHABET[(n >> 6) & 63] : '=';
    out += i + 2 < bytes.length ? ALPHABET[n & 63] : '=';
  }
  return out;
}

/** 照拒绝里的 `data.received` 接着传的那两种；交回从哪接着，别的交 `null`。 @param {any} err */
function resumeAt(err) {
  const at = err?.data?.received;
  return (err?.reason === 'upload_offset' || err?.reason === 'upload_incomplete') && Number.isInteger(at) ? at : null;
}

/**
 * 传一个附件。
 * @param {(method: string, params: any) => Promise<any>} request 发给核心
 * @param {{name: string, size: number}} file
 * @param {string|null} mediaType 浏览器给的媒体类型（不合规矩的、空的给 `null`，核心照内容认）
 * @param {(offset: number, length: number) => Promise<Uint8Array>} read 读一段字节
 * @param {{chunk: number, tries: number}} limits 一块多少字节（核心收的上限 512 KiB）、连着几次没往前走就不再接着传
 * @returns {Promise<any>} `blob.close` 的回应
 */
export async function upload(request, file, mediaType, read, { chunk, tries }) {
  const { upload: id } = await request('blob.open', { name: file.name, size: file.size, ...(mediaType ? { media_type: mediaType } : {}) });
  let offset = 0;
  let stuck = 0;
  /** 接着传：往前走了的重新数，连着几次没往前走的照原样抛 */
  const resume = (err) => {
    const at = resumeAt(err);
    if (at === null) throw err;
    stuck = at > offset ? 0 : stuck + 1;
    if (stuck >= tries) throw err;
    offset = at;
  };
  for (;;) {
    while (offset < file.size) {
      const bytes = await read(offset, Math.min(chunk, file.size - offset));
      try {
        const got = await request('blob.write', { upload: id, offset, data: toBase64(bytes) });
        offset = got.received;
        stuck = 0;
      } catch (err) {
        resume(err);
      }
    }
    try {
      return await request('blob.close', { upload: id });
    } catch (err) {
      resume(err);
    }
  }
}

/**
 * 一个接一个地办：交进来的事排队，前一个完了（成了、出错都算）才办下一个。附件同时放进来好几个时用：核心一条连接同时开的上传
 * 有上限（第 5 个回 `too_many_uploads`），同一条连接上的请求核心本来就一条条办，排着不慢。
 * @returns {<T>(fn: () => Promise<T>) => Promise<T>}
 */
export function serial() {
  /** @type {Promise<unknown>} */
  let last = Promise.resolve();
  return (fn) => {
    const next = last.then(fn, fn);
    last = next.catch(() => {});
    return next;
  };
}
