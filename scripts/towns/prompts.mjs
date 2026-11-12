// 设定里的提示词调校：读、存、看拼好的模板、拿一句话试生成一个人物。逻辑在 host::prompts，
// 经 town-map 子进程调用；手机上的管理服务直接调同一份（crates/manager/src/routes.rs），两边路径和回答一样。
// 写故事的试一试走 /story（请求里带 prompt）。
import { worker } from './workers.mjs'

export function createPromptRoutes({ source, config, serial, mockLlm }) {
  const run = (mode, input, options) => worker(source, config, mode, input, options)
  return [
    ['GET', /^\/api\/towns\/prompts$/, () => run('prompts', {}, { timeout: 10000 })],
    [
      'POST',
      /^\/api\/towns\/prompts$/,
      ({ body }) => serial('prompts', () => run('save-prompts', body, { timeout: 10000 })),
    ],
    [
      'POST',
      /^\/api\/towns\/prompts\/preview$/,
      ({ body }) => run('preview-prompt', body, { timeout: 10000 }),
    ],
    // mock 只由管理服务自己定，页面带来的不算
    [
      'POST',
      /^\/api\/towns\/prompts\/character$/,
      ({ body, signal }) =>
        run('try-character', { ...body, mock: mockLlm }, { signal, timeout: 120000 }),
    ],
  ]
}
