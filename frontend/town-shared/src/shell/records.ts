// 菜单里的「记录」：把这个小镇记下的东西整份下载——说过的话、想法、世界日志、村民 Agent 的轨迹
// （发给模型的请求、模型的回复、工具调用和结果）。世界进程一直在往小镇目录里记
// （crates/host/src/transcript.rs），这里向管理服务要回来（/api/towns/<小镇>/transcript），
// 在页面里拼成一个文件：JSON（一个对象，带说明和条目）、JSONL（一行一条）或者给人读的文字。
import { button, el } from '../dom'
import { clock as clockOf } from '../format'
import { errorText } from '../format'
import './records.css'

type Kind = 'chat' | 'thought' | 'event' | 'trace'
type Format = 'json' | 'jsonl' | 'text'
/** 一条记录：种类、现实时间（毫秒）、世界编号、世界里的分钟，其余随种类 */
export type RecordLine = { kind: Kind; at: number; world: string; minutes?: number } & Record<
  string,
  unknown
>

const KINDS: { kind: Kind; label: string; hint: string }[] = [
  { kind: 'chat', label: '对话', hint: '私聊里的每一句、玩法里的发言' },
  { kind: 'thought', label: '想法', hint: '每次做决定时心里想的和来由' },
  { kind: 'event', label: '世界日志', hint: '走动、活动、检定、战斗这些发生过的事' },
  {
    kind: 'trace',
    label: '模型轨迹',
    hint: '发给模型的请求、回复、工具调用和结果，很大；记不记完整题面看设定里的「日志」',
  },
]
const FORMATS: { format: Format; label: string; ext: string; mime: string }[] = [
  { format: 'json', label: 'JSON', ext: 'json', mime: 'application/json' },
  { format: 'jsonl', label: 'JSONL（一行一条）', ext: 'jsonl', mime: 'application/x-ndjson' },
  { format: 'text', label: '文字（给人读）', ext: 'md', mime: 'text/markdown' },
]

/** 管理服务给的原文（每种一段，每行一条 JSON）拆成条目，按世界里的先后排；写到一半的最后一行不要 */
export function parseRecords(kinds: Partial<Record<Kind, string>>): RecordLine[] {
  const lines: RecordLine[] = []
  for (const [kind, text] of Object.entries(kinds) as [Kind, string | undefined][])
    for (const row of (text ?? '').split('\n')) {
      if (!row.trim()) continue
      try {
        const line = JSON.parse(row) as RecordLine
        // 轨迹是世界进程的日志原样给的（{trace, model_snapshot}），没有种类和时刻：在这里补上。
        // 它不记是哪个世界，算在记录里最新的那个世界上（重置过世界的小镇，旧世界的轨迹会排到新世界里）
        if (!line.kind) {
          const trace = (line as { trace?: { sim_minutes?: number } }).trace
          Object.assign(line, { kind, at: 0, world: '', minutes: trace?.sim_minutes ?? 0 })
        }
        lines.push(line)
      } catch {
        // 世界进程正写到一半的那一行
      }
    }
  const latest = lines.reduce<RecordLine | null>(
    (best, l) => (l.world && (!best || l.at > best.at) ? l : best),
    null
  )
  for (const l of lines) if (!l.world) l.world = latest?.world ?? '（只有轨迹）'
  // 同一个世界里按世界分钟排；不同的世界（重置过）按现实时间分先后
  const born = new Map<string, number>()
  for (const l of lines) born.set(l.world, Math.min(born.get(l.world) ?? Infinity, l.at))
  return lines
    .map((line, i) => ({ line, i }))
    .sort(
      (a, b) =>
        (born.get(a.line.world) ?? 0) - (born.get(b.line.world) ?? 0) ||
        (a.line.minutes ?? 0) - (b.line.minutes ?? 0) ||
        a.line.at - b.line.at ||
        a.i - b.i
    )
    .map(({ line }) => line)
}

const brief = (value: unknown, max: number) => {
  const text = typeof value === 'string' ? value : JSON.stringify(value)
  return text.length > max ? `${text.slice(0, max)}…` : text
}

/** 给人读的一行 */
function lineText(l: RecordLine): string {
  const when = typeof l.clock === 'string' && l.clock ? l.clock : clockOf(l.minutes ?? 0)
  const name = typeof l.name === 'string' ? l.name : ''
  if (l.kind === 'chat') {
    const withWhom = Array.isArray(l.with) ? (l.with as string[]).join('、') : ''
    const where = l.occasion ? `（${l.occasion as string}）` : withWhom ? ` → ${withWhom}` : ''
    return `[${when}] 【对话】${name}${where}：${l.text as string}`
  }
  if (l.kind === 'thought') {
    const trigger = (l.trigger as { text?: string } | undefined)?.text
    return `[${when}] 【想法】${name}${trigger ? `（${trigger}）` : ''}：${l.text as string}`
  }
  if (l.kind === 'event') return `[${when}] ${l.text as string}`
  const t = (l.trace ?? {}) as {
    actor_id?: string
    event?: string
    request_id?: number
    data?: unknown
  }
  const model = (l.model_snapshot as { model?: string } | null)?.model
  return `[${when}] 【轨迹】${t.actor_id ?? ''} ${t.event ?? ''}${t.request_id ? ` #${t.request_id}` : ''}${model ? `（${model}）` : ''} ${brief(t.data, 400)}`
}

/** 拼成要下载的文件内容 */
export function renderRecords(
  lines: RecordLine[],
  format: Format,
  town: { id: string; name: string }
) {
  if (format === 'jsonl') return lines.map((l) => JSON.stringify(l)).join('\n') + '\n'
  const counts: Record<string, number> = {}
  for (const l of lines) counts[l.kind] = (counts[l.kind] ?? 0) + 1
  if (format === 'json')
    return JSON.stringify(
      { town, exported_at: new Date().toISOString(), counts, items: lines },
      null,
      1
    )
  const out = [`# ${town.name} · 记录`, '']
  let world = ''
  for (const l of lines) {
    if (l.world !== world) {
      world = l.world
      out.push('', `## 世界 ${world}（${new Date(l.at).toLocaleString()} 起）`, '')
    }
    out.push(lineText(l))
  }
  return out.join('\n') + '\n'
}

/** 存成文件；返回字节数 */
function save(name: string, mime: string, content: string) {
  const blob = new Blob([content], { type: `${mime};charset=utf-8` })
  const url = URL.createObjectURL(blob)
  const link = el('a')
  link.href = url
  link.download = name
  link.hidden = true
  document.body.append(link)
  link.click()
  setTimeout(() => {
    link.remove()
    URL.revokeObjectURL(url)
  }, 60_000)
  return blob.size
}

/** 打开「记录」对话框：勾要哪几种、选格式，点下载 */
export function openRecords(town: { id: string; name: string }) {
  document.querySelector('dialog.records')?.remove()
  const dialog = el('dialog', '', 'records')
  const chosen = new Set<Kind>(['chat', 'thought', 'event'])
  let format: Format = 'json'
  const status = el(
    'p',
    '记录一直在记，随时能下载；世界开着时下载到的是到此刻为止的。',
    'records-status'
  )
  const kinds = el('div', '', 'records-kinds')
  for (const k of KINDS) {
    const box = el('input')
    box.type = 'checkbox'
    box.checked = chosen.has(k.kind)
    box.onchange = () => (box.checked ? chosen.add(k.kind) : chosen.delete(k.kind))
    const row = el('label')
    row.append(box, el('b', k.label), el('small', k.hint))
    kinds.append(row)
  }
  const formats = el('div', '', 'records-formats')
  for (const f of FORMATS) {
    const radio = el('input')
    radio.type = 'radio'
    radio.name = 'records-format'
    radio.checked = f.format === format
    radio.onchange = () => (format = f.format)
    const row = el('label')
    row.append(radio, f.label)
    formats.append(row)
  }
  const go = button('下载', () => void download(), 'primary')
  async function download() {
    if (!chosen.size) {
      status.textContent = '先勾上要下载的种类。'
      return
    }
    go.disabled = true
    status.textContent = '正在取记录…'
    try {
      const response = await fetch(
        `/api/towns/${encodeURIComponent(town.id)}/transcript?kinds=${[...chosen].join(',')}`,
        { headers: { 'X-Town-Client': '1' } }
      )
      const data = await response.json().catch(() => null)
      if (!response.ok) throw Error(data?.error ?? `小镇服务返回 ${response.status}`)
      const lines = parseRecords(data.kinds ?? {})
      if (!lines.length) {
        status.textContent = '还没有记下东西：让世界跑一阵再来。'
        return
      }
      const f = FORMATS.find((x) => x.format === format) ?? FORMATS[0]
      // 文件名只用英文和数字：有的浏览器不认中文的下载名，会存成没有后缀的「download」；小镇的名字写在文件里
      const stamp = new Date().toISOString().slice(0, 16).replace(/[-:T]/g, '')
      const content = renderRecords(lines, format, town)
      const bytes = save(`records-${town.id.slice(0, 18)}-${stamp}.${f.ext}`, f.mime, content)
      const parts = KINDS.filter((k) => chosen.has(k.kind))
        .map((k) => `${k.label} ${lines.filter((l) => l.kind === k.kind).length} 条`)
        .join('，')
      status.textContent = `已下载：${parts}（${(bytes / 1024 / 1024).toFixed(1)} MB）。`
    } catch (error) {
      status.textContent = `没下载成：${errorText(error)}`
    } finally {
      go.disabled = false
    }
  }
  const head = el('header')
  head.append(
    el('strong', '记录'),
    button('关闭', () => dialog.close())
  )
  const body = el('div', '', 'records-body')
  body.append(el('h4', '要哪几种'), kinds, el('h4', '格式'), formats, status)
  const foot = el('footer')
  foot.append(go)
  dialog.append(head, body, foot)
  dialog.addEventListener('close', () => dialog.remove())
  document.body.append(dialog)
  dialog.showModal()
}
