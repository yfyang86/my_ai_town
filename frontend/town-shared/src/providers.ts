// 全部走 OpenAI 兼容传输——base_url 不含 /chat/completions 后缀（host 拼接）。
// 预置清单按各服务商 2026-11 的官方模型文档维护；玩家添加的模型存在配置里，和预置一起候选。
import type { ActiveModelChoice, ProviderConfig, ProviderView } from './protocol'

export interface ProviderPreset {
  id: string
  name: string
  baseUrl: string
  needsKey: boolean
  models: string[]
  defaultModel: string
}

export const PRESETS: ProviderPreset[] = [
  {
    id: 'deepseek',
    name: 'DeepSeek',
    baseUrl: 'https://api.deepseek.com',
    needsKey: true,
    models: ['deepseek-flash', 'deepseek-v4-pro'],
    defaultModel: 'deepseek-flash',
  },
  {
    id: 'kimi',
    name: 'Kimi',
    baseUrl: 'https://api.moonshot.cn/v1',
    needsKey: true,
    models: ['kimi-k3.1-pro', 'kimi-k3.1-pro-highspeed', 'kimi-k3'],
    defaultModel: 'kimi-k3.1-pro',
  },
  {
    id: 'zhipu-glm',
    name: '智谱 GLM',
    baseUrl: 'https://open.bigmodel.cn/api/paas/v4',
    needsKey: true,
    models: [
      'glm-5.3',
      'glm-5.3-flash',
      'glm-5.3-flashx',
      'glm-5.2',
      'glm-5.1',
      'glm-5',
      'glm-4.7',
      'glm-5v-turbo',
    ],
    defaultModel: 'glm-5.3',
  },
  {
    id: 'minimax',
    name: 'MiniMax',
    baseUrl: 'https://api.minimax.cn/v1',
    needsKey: true,
    models: ['MiniMax-M3.1-Flash', 'MiniMax-M3'],
    defaultModel: 'MiniMax-M3.1-Flash',
  },
  {
    id: 'volcengine-ark',
    name: '火山方舟',
    baseUrl: 'https://ark.cn-beijing.volces.com/api/v3',
    needsKey: true,
    models: ['doubao-seed-2-1-turbo-260628', 'glm-5-3-flash-260828'],
    defaultModel: 'doubao-seed-2-1-turbo-260628',
  },
  {
    id: 'alibaba-bailian',
    name: '阿里百炼',
    baseUrl: 'https://dashscope.aliyuncs.com/compatible-mode/v1',
    needsKey: true,
    models: [
      'qwen3.7-plus',
      'qwen3.8-max',
      'qwen3.8-flash',
      'qwen3.7-max',
      'qwen3.7-flash',
      'qwen3.6-plus',
      'qwen3.6-flash',
      'qwen3.5-plus',
      'qwen3.5-flash',
    ],
    defaultModel: 'qwen3.7-plus',
  },
  {
    id: 'xiaomi-mimo',
    name: '小米 MiMo',
    baseUrl: 'https://api.xiaomimimo.com/v1',
    needsKey: true,
    models: ['mimo-v2.6-pro', 'mimo-v2.6-pro-ultraspeed', 'mimo-v2.6-flash'],
    defaultModel: 'mimo-v2.6-pro',
  },
  {
    id: '302-ai',
    name: '302.AI',
    baseUrl: 'https://api.302.ai/v1',
    needsKey: true,
    models: [],
    defaultModel: '',
  },
  {
    // 老项目走 Ollama 原生 API；本项目统一 OpenAI 兼容端点
    id: 'ollama',
    name: 'Ollama（本地）',
    baseUrl: 'http://127.0.0.1:11434/v1',
    needsKey: false,
    models: [],
    defaultModel: '',
  },
  {
    id: 'ollama-cloud',
    name: 'Ollama Cloud',
    baseUrl: 'https://ollama.com/v1',
    needsKey: true,
    models: [],
    defaultModel: '',
  },
  {
    id: 'lm-studio',
    name: 'LM Studio（本地）',
    baseUrl: 'http://127.0.0.1:1234/v1',
    needsKey: false,
    models: [],
    defaultModel: '',
  },
  {
    id: 'custom',
    name: '自定义（OpenAI 兼容）',
    baseUrl: '',
    needsKey: true,
    models: [],
    defaultModel: '',
  },
  {
    // 随游戏带的小模型，世界进程在本机跑；应用带着它时服务端才把它列进配置
    id: 'builtin',
    name: '内置模型',
    baseUrl: '',
    needsKey: false,
    models: ['小镇村民'],
    defaultModel: '小镇村民',
  },
]

type ProviderModels = Pick<ProviderView, 'model'> & Partial<Pick<ProviderView, 'models'>>

function modelNames(models: (string | undefined)[]): string[] {
  return [
    ...new Set(models.map((model) => model?.trim()).filter((model): model is string => !!model)),
  ]
}

/** 模型输入框和下拉框共用：当前模型排最前，再列服务记着的、玩家添加的和预置候选。 */
export function modelCandidates(id: string, conf?: ProviderModels): string[] {
  const preset = PRESETS.find((p) => p.id === id)
  return modelNames([
    conf?.model,
    ...(conf?.models ?? []),
    ...(preset?.models ?? []),
    preset?.defaultModel,
  ])
}

/** 首页和请村民时共用候选；玩家手填的当前模型即使不在目录里也保留。 */
export function quickModelNames(
  provider: string,
  current?: string,
  conf?: ProviderModels
): string[] {
  return modelNames([current, ...modelCandidates(provider, conf)])
}

/** 选了这个服务时模型名先填什么：「模型服务」里给它填过的；没填过时默认模型用着这个服务就用它的，
 *  再不然用预置的缺省；都没有为空串 */
export function providerModel(
  id: string,
  conf?: Pick<ProviderView, 'model'>,
  active?: ActiveModelChoice | null
): string {
  return (
    conf?.model ||
    (active?.provider === id ? active.model : '') ||
    PRESETS.find((p) => p.id === id)?.defaultModel ||
    ''
  )
}

/** 服务端按整表保存：没改的服务原样写回去，模型名和玩家添加的候选都不能丢。 */
export function providerRow(p: ProviderView): ProviderConfig {
  const models = modelNames(p.models ?? [])
  return {
    id: p.id,
    name: p.name,
    base_url: p.base_url,
    ...(p.model ? { model: p.model } : {}),
    ...(models.length ? { models } : {}),
  }
}

/** 选中一个模型时把它设为服务缺省；不在预置目录里的同时存进玩家候选。 */
export function rememberProviderModel(p: ProviderConfig, model: string): ProviderConfig {
  const preset = PRESETS.find((item) => item.id === p.id)
  const models = modelNames([
    ...(p.models ?? []),
    ...(preset?.models.includes(model) ? [] : [model]),
  ])
  return {
    ...p,
    model,
    ...(models.length ? { models } : { models: [] }),
  }
}

/** 内置模型：不用地址和密钥，只给村民过日子用，不写人设、不建镇 */
export const BUILTIN = 'builtin'

/** 本机或局域网地址上的服务（Ollama、LM Studio 这类）不用密钥；和服务端的判断一致 */
export function localEndpoint(url: string): boolean {
  let host = ''
  try {
    host = new URL(url).hostname.replace(/^\[|\]$/g, '').toLowerCase()
  } catch {
    return false
  }
  return (
    host === 'localhost' ||
    host.endsWith('.localhost') ||
    host.endsWith('.local') ||
    host === '::1' ||
    /^(127\.|10\.|192\.168\.|169\.254\.|0\.0\.0\.0$)/.test(host) ||
    /^172\.(1[6-9]|2\d|3[01])\./.test(host)
  )
}

/** 要不要填密钥：预置里不用的不要，地址填在本机或局域网的（比如自定义地址接本机模型）也不要 */
export function needsKey(preset: ProviderPreset | undefined, baseUrl = ''): boolean {
  return !!preset?.needsKey && !localEndpoint(baseUrl)
}
