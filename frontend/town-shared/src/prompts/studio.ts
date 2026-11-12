// 设定里的「提示词」：写故事、生成人物这两处给模型的提示词，在这里调。
// - 默认：随应用带的那一份；
// - 风格模式：挑一套预设，再逐段改——身份、背景、文风、例子、不要、自查，输出格式不用管；
// - 专业模式：整份模板自己写，占位符由世界换成实际的数；
// - 试一试、比一比：同一段故事或一句话，拿两份提示词各跑一次，并排看。
// 设置所有小镇共用，经管理服务的 /api/towns/prompts 读写（crates/host/src/prompts.rs），怎么拼在 crates/agent/src/tuning.rs。
import { button, el } from '../dom'
import { errorText } from '../format'
import { pixelIcon } from '../game/pixel'
import { gameIcon } from '../game/icons'
import { segmented } from '../settings/controls'
import {
  CHARACTER_PRESETS,
  EMPTY_MODULES,
  MODULE_FIELDS,
  STORY_PRESETS,
  type PromptModules,
  type PromptPreset,
} from './presets'
import './studio.css'

type Target = 'story' | 'character'
type Mode = 'default' | 'style' | 'pro'
interface Tuning {
  mode: Mode
  modules: PromptModules
  pro: string
  preset: string
}
type Settings = Record<Target, Tuning>
interface About {
  template: string
  placeholders: { mark: string; means: string }[]
  warnings: string[]
}
interface View {
  settings: Settings
  defaults: Record<Target, About>
  limits: { module: number; pro: number }
}

/** 写故事要带的地点种类和小镇风格（建镇生成器那边的表）；由页面入口登记，用到时才取 */
type StoryOptions = { kinds: unknown[]; presets: unknown[] }
let storyOptions: (() => Promise<StoryOptions>) | null = null
export function setPromptStoryOptions(load: () => Promise<StoryOptions>) {
  storyOptions = load
}

const TARGETS: { value: Target; label: string; what: string; input: string }[] = [
  {
    value: 'story',
    label: '写故事',
    what: '选择小镇页「写一个故事」时，把故事或提纲排成角色、节拍和里程碑用的提示词。',
    input: '一段故事、一个提纲，或者只是一句设定',
  },
  {
    value: 'character',
    label: '生成人物',
    what: '村民库里「用模型生成」时，把一句话描述写成一位村民用的提示词。',
    input: '一句话描述这个人，比如：退休的灯塔看守，话少，养了一只独眼猫',
  },
]
const MODES: { value: Mode; label: string; what: string }[] = [
  { value: 'default', label: '默认', what: '用随应用带的提示词，什么都不用管。' },
  {
    value: 'style',
    label: '风格模式',
    what: '挑一套风格，再逐段改。只调味道，模型交回来的格式不变，不会排不出来。',
  },
  {
    value: 'pro',
    label: '专业模式',
    what: '整份提示词自己写，连输出格式在内。改坏了格式模型的回复可能用不了，随时能恢复默认。',
  },
]

const copy = <T>(value: T): T => JSON.parse(JSON.stringify(value)) as T
const presetsOf = (target: Target) => (target === 'story' ? STORY_PRESETS : CHARACTER_PRESETS)

async function call<T>(path: string, body?: unknown, timeout = 30000): Promise<T> {
  const response = await fetch('/api/towns' + path, {
    method: body === undefined ? 'GET' : 'POST',
    headers: {
      'X-Town-Client': '1',
      ...(body !== undefined && { 'Content-Type': 'application/json' }),
    },
    ...(body !== undefined && { body: JSON.stringify(body) }),
    signal: AbortSignal.timeout(timeout),
  })
  const data = await response.json().catch(() => null)
  if (!response.ok) throw Error(data?.error ?? `小镇服务返回 ${response.status}`)
  return data as T
}

let dialog: HTMLDialogElement | null = null

/** 打开「提示词」。同一时间只有一份 */
export function openPromptStudio() {
  dialog?.remove()
  const root = el('dialog', '', 'prompt-studio')
  dialog = root
  root.setAttribute('aria-labelledby', 'prompt-studio-title')

  let view: View | null = null
  let draft: Settings | null = null
  let target: Target = 'story'
  let tab: 'tune' | 'compare' = 'tune'
  let status = ''
  let busy = false

  const head = el('header', '', 'ps-head')
  const back = button('', () => close(), 'ps-back')
  back.append(gameIcon('back', true))
  back.setAttribute('aria-label', '返回')
  const title = el('h1', '提示词')
  title.id = 'prompt-studio-title'
  const targetSeg = segmented('调哪一处', TARGETS, (v) => {
    target = v as Target
    render()
  })
  const mark = el('span', '', 'ps-mark')
  mark.append(pixelIcon('scroll'))
  head.append(back, mark, title, targetSeg.el)
  const tabSeg = segmented(
    '做什么',
    [
      { value: 'tune', label: '调提示词' },
      { value: 'compare', label: '试一试 · 比一比' },
    ],
    (v) => {
      tab = v as typeof tab
      render()
    }
  )
  tabSeg.el.classList.add('ps-tabs')
  const body = el('div', '', 'ps-body')
  const foot = el('footer', '', 'ps-foot')
  root.append(head, tabSeg.el, body, foot)
  document.body.append(root)

  const dirty = () => !!view && !!draft && JSON.stringify(view.settings) !== JSON.stringify(draft)
  function close() {
    if (dirty() && !confirm('提示词改了还没保存，放弃这些修改吗？')) return
    root.close()
  }
  root.addEventListener('cancel', (event) => {
    event.preventDefault()
    close()
  })
  root.addEventListener('close', () => {
    root.remove()
    if (dialog === root) dialog = null
  })

  async function load() {
    status = '正在读取…'
    render()
    try {
      view = await call<View>('/prompts')
      draft = copy(view.settings)
      status = ''
    } catch (error) {
      status = `读不了提示词设置：${errorText(error)}`
    }
    render()
  }
  async function save() {
    if (!draft) return
    busy = true
    status = '正在保存…'
    render()
    try {
      view = await call<View>('/prompts', { settings: draft })
      draft = copy(view.settings)
      const warnings = view.defaults[target].warnings
      status = warnings.length
        ? `已保存。留意：${warnings.join('；')}`
        : '已保存，下一次生成就用它。'
    } catch (error) {
      status = `没保存成：${errorText(error)}`
    }
    busy = false
    render()
  }

  // ---------- 调提示词 ----------

  function area(value: string, rows: number, max: number, set: (v: string) => void, mono = false) {
    const box = el('div', '', 'ps-area')
    const input = el('textarea')
    input.value = value
    input.rows = rows
    input.maxLength = max
    input.spellcheck = false
    if (mono) input.classList.add('mono')
    const count = el('small', `${value.length} / ${max}`)
    input.oninput = () => {
      set(input.value)
      count.textContent = `${input.value.length} / ${max}`
      renderFoot()
    }
    box.append(input, count)
    return { box, input }
  }

  /** 拼好的整份模板：点开才向管理服务要 */
  function composed(tuning: () => Tuning, label = '看拼好的整份提示词') {
    const box = el('details', '', 'ps-fold')
    const pre = el('pre', '', 'ps-pre')
    box.append(el('summary', label), pre)
    box.ontoggle = async () => {
      if (!box.open) return
      pre.textContent = '正在拼…'
      try {
        const reply = await call<{ template: string }>('/prompts/preview', {
          target,
          tuning: tuning(),
        })
        pre.textContent =
          reply.template +
          (target === 'story' ? '\n提纲：\n（你写的故事或提纲）' : '描述：（你写的一句话）')
      } catch (error) {
        pre.textContent = `拼不出来：${errorText(error)}`
      }
    }
    return box
  }

  function tuneView(v: View, d: Settings) {
    const t = d[target]
    const about = TARGETS.find((x) => x.value === target)!
    const intro = el('section', '', 'ps-card')
    const modeSeg = segmented('用哪一种', MODES, (m) => {
      t.mode = m as Mode
      // 头一次进专业模式：把此刻拼好的那一份放进来当起点
      if (t.mode === 'pro' && !t.pro.trim()) t.pro = v.defaults[target].template.trimEnd()
      render()
    })
    modeSeg.set(t.mode)
    intro.append(
      el('h2', about.label),
      el('p', about.what, 'ps-hint'),
      modeSeg.el,
      el('p', MODES.find((m) => m.value === t.mode)!.what, 'ps-hint')
    )
    const parts: HTMLElement[] = [intro]
    if (t.mode === 'default') {
      const fold = el('details', '', 'ps-fold')
      const pre = el('pre', v.defaults[target].template, 'ps-pre')
      fold.append(el('summary', '看默认的提示词'), pre)
      parts.push(fold)
    } else if (t.mode === 'style') {
      const pick = el('section', '', 'ps-card')
      const chips = el('div', '', 'ps-presets')
      for (const p of presetsOf(target)) {
        const chip = button('', () => {
          t.modules = copy(p.modules)
          t.preset = p.id
          render()
        })
        chip.className = 'ps-preset'
        chip.setAttribute('aria-pressed', String(t.preset === p.id))
        chip.append(el('b', p.name), el('small', p.hint))
        chips.append(chip)
      }
      const blank = button('', () => {
        t.modules = copy(EMPTY_MODULES)
        t.preset = ''
        render()
      })
      blank.className = 'ps-preset'
      blank.setAttribute('aria-pressed', String(!t.preset))
      blank.append(el('b', '自己写'), el('small', '从空白开始'))
      chips.append(blank)
      pick.append(
        el('h3', '从哪一套开始'),
        el('p', '选一套会换掉下面六段，之后可以逐段改。', 'ps-hint'),
        chips
      )
      const modules = el('section', '', 'ps-card')
      modules.append(el('h3', '逐段调'))
      for (const f of MODULE_FIELDS) {
        const field = el('label', '', 'ps-field')
        const name = el('span', '', 'ps-field-name')
        name.append(el('b', f.label), el('small', f.hint))
        const a = area(t.modules[f.key], f.key === 'role' ? 2 : 4, v.limits.module, (text) => {
          t.modules[f.key] = text
        })
        a.input.placeholder = f.placeholder
        field.append(name, a.box)
        modules.append(field)
      }
      parts.push(
        pick,
        modules,
        composed(() => t)
      )
    } else {
      const pro = el('section', '', 'ps-card')
      const marks = el('p', '', 'ps-marks')
      marks.append('占位符（原样写在模板里，发出去时换成实际的内容）：')
      for (const p of v.defaults[target].placeholders) {
        const chip = el('span', '', 'ui-chip')
        chip.append(el('code', p.mark), ` ${p.means}`)
        marks.append(chip)
      }
      const a = area(t.pro, 22, v.limits.pro, (text) => (t.pro = text), true)
      const tools = el('div', '', 'ps-tools')
      tools.append(
        button('恢复成默认的那一份', () => {
          t.pro = v.defaults[target].template.trimEnd()
          render()
        }),
        button('用风格模式拼好的那一份', async () => {
          try {
            const reply = await call<{ template: string }>('/prompts/preview', {
              target,
              tuning: { ...t, mode: 'style' },
            })
            t.pro = reply.template.trimEnd()
            render()
          } catch (error) {
            status = `拼不出来：${errorText(error)}`
            renderFoot()
          }
        })
      )
      pro.append(
        el('h3', '整份模板'),
        el(
          'p',
          target === 'story'
            ? '模板写到提纲之前为止：提纲（开着第五版规则时还有规则那一段）会接在后面。'
            : '模板写到「描述：」之前为止：那一句话会接在后面。',
          'ps-hint'
        ),
        marks,
        a.box,
        tools
      )
      parts.push(pro)
    }
    return parts
  }

  // ---------- 试一试 · 比一比 ----------

  const inputs: Record<Target, string> = { story: '', character: '' }
  let people = 6
  let days = 1
  let rules = false
  const sides: [string, string] = ['default', 'editing']
  type Outcome = { label: string; seconds: number; data?: Record<string, unknown>; error?: string }
  let results: (Outcome | null)[] = [null, null]
  let running = false

  function candidates(v: View, d: Settings) {
    const list: { id: string; label: string; tuning: Tuning }[] = [
      {
        id: 'default',
        label: '默认',
        tuning: { mode: 'default', modules: copy(EMPTY_MODULES), pro: '', preset: '' },
      },
      { id: 'editing', label: '正在改的这一份', tuning: d[target] },
      { id: 'saved', label: '已保存的那一份', tuning: v.settings[target] },
    ]
    for (const p of presetsOf(target) as PromptPreset[])
      list.push({
        id: `preset:${p.id}`,
        label: `风格：${p.name}`,
        tuning: { mode: 'style', modules: copy(p.modules), pro: '', preset: p.id },
      })
    return list
  }

  async function runOne(tuning: Tuning, label: string): Promise<Outcome> {
    const started = performance.now()
    const seconds = () => Math.round((performance.now() - started) / 100) / 10
    try {
      const data =
        target === 'story'
          ? await call<Record<string, unknown>>(
              '/story',
              {
                outline: inputs.story,
                cast: people,
                days,
                ...(rules && { rules: 'srd5' }),
                ...(await (storyOptions?.() ?? Promise.resolve({ kinds: [], presets: [] }))),
                prompt: tuning,
              },
              300000
            )
          : await call<Record<string, unknown>>(
              '/prompts/character',
              { brief: inputs.character, tuning },
              120000
            )
      return { label, seconds: seconds(), data }
    } catch (error) {
      return { label, seconds: seconds(), error: errorText(error) }
    }
  }

  function line(label: string, text: unknown) {
    const row = el('p', '', 'ps-line')
    row.append(el('b', label), String(text ?? ''))
    return row
  }
  function storyResult(draft: Record<string, any>) {
    const out: HTMLElement[] = [
      el('h4', draft.title ?? '（没有名字）'),
      el('p', draft.premise ?? '', 'ps-premise'),
    ]
    const cast = el('ol', '', 'ps-cast')
    for (const c of draft.cast ?? []) {
      const item = el('li')
      item.append(el('b', `${c.name} · ${c.role}`), el('span', c.intro ?? ''))
      item.append(line('想要', c.goal))
      if (c.secret) item.append(line('秘密', c.secret))
      if (c.speech) item.append(line('说话', c.speech))
      cast.append(item)
    }
    const beats = el('ol', '', 'ps-beats')
    for (const b of draft.beats ?? []) {
      const clock = `${String(Math.floor(b.minute / 60)).padStart(2, '0')}:${String(b.minute % 60).padStart(2, '0')}`
      const item = el('li')
      item.append(
        el('b', `${draft.ends_day > 1 ? `第 ${b.day} 天 ` : ''}${clock} ${b.title}`),
        el('span', b.announce ?? '')
      )
      if (b.checks?.length)
        item.append(line('检定', b.checks.map((c: any) => `${c.title}（难度 ${c.dc}）`).join('、')))
      if (b.encounter) item.append(line('遭遇战', b.encounter.title))
      beats.append(item)
    }
    const marks = el('ul', '', 'ps-marks-list')
    for (const m of draft.milestones ?? []) {
      const item = el('li')
      item.append(el('b', m.title), el('span', m.detail ?? ''))
      marks.append(item)
    }
    out.push(
      el('h5', `角色（${draft.cast?.length ?? 0}）`),
      cast,
      el('h5', `节拍（${draft.beats?.length ?? 0}）`),
      beats
    )
    out.push(el('h5', `里程碑（${draft.milestones?.length ?? 0}）`), marks)
    if (draft.epilogue) out.push(line('收场', draft.epilogue))
    for (const note of draft.notes ?? []) out.push(el('p', `· ${note}`, 'ps-hint'))
    return out
  }
  function characterResult(f: Record<string, any>) {
    const tags = el('p', '', 'ps-tags')
    for (const t of f.tags ?? []) tags.append(el('span', t, 'ui-chip'))
    return [
      el('h4', `${f.name ?? ''} · ${[f.age, f.gender, f.occupation].filter(Boolean).join(' · ')}`),
      el('p', f.intro ?? '', 'ps-premise'),
      tags,
      line('性格', f.personality),
      line('说话', f.speech),
      line('来历', f.background),
      line('想要', f.drives),
      line('喜好', f.interests),
    ]
  }

  function compareView(v: View, d: Settings) {
    const about = TARGETS.find((x) => x.value === target)!
    const list = candidates(v, d)
    const input = el('section', '', 'ps-card')
    const text = area(
      inputs[target],
      target === 'story' ? 6 : 2,
      target === 'story' ? 6000 : 300,
      (t) => {
        inputs[target] = t
        go.disabled = running || !t.trim()
      }
    )
    text.input.placeholder = about.input
    input.append(el('h2', '拿什么来试'), text.box)
    if (target === 'story') {
      const row = el('div', '', 'ps-row')
      const pick = (
        label: string,
        value: number,
        options: number[],
        unit: string,
        set: (n: number) => void
      ) => {
        const select = el('select')
        for (const n of options) {
          const o = el('option', `${n} ${unit}`)
          o.value = String(n)
          select.append(o)
        }
        select.value = String(value)
        select.onchange = () => set(Number(select.value))
        const box = el('label', '', 'ps-pick')
        box.append(el('span', label), select)
        return box
      }
      const withRules = el('label', '', 'ps-check')
      const check = el('input')
      check.type = 'checkbox'
      check.checked = rules
      check.onchange = () => (rules = check.checked)
      withRules.append(check, '带第五版规则')
      row.append(
        pick('角色', people, [2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12], '个', (n) => (people = n)),
        pick('历时', days, [1, 2, 3], '天', (n) => (days = n)),
        withRules
      )
      input.append(row)
    }
    const versus = el('div', '', 'ps-versus')
    sides.forEach((side, i) => {
      const select = el('select')
      for (const c of list) {
        const o = el('option', c.label)
        o.value = c.id
        select.append(o)
      }
      select.value = list.some((c) => c.id === side) ? side : 'default'
      sides[i] = select.value
      select.onchange = () => (sides[i] = select.value)
      const box = el('label', '', 'ps-pick')
      box.append(el('span', i === 0 ? '左边用' : '右边用'), select)
      versus.append(box)
      if (i === 0) versus.append(el('i', '对', 'ps-vs'))
    })
    const go = button(
      running ? '两边正在各跑一次…' : '各跑一次，并排看',
      () => void run(),
      'primary'
    )
    go.disabled = running || !inputs[target].trim()
    input.append(
      versus,
      el(
        'p',
        target === 'story'
          ? '两边各调一次模型，同时跑；排一个故事要一两分钟。结果只是看看，不建小镇。'
          : '两边各调一次模型，同时跑。结果只是看看，不存进村民库。',
        'ps-hint'
      ),
      go
    )
    async function run() {
      const chosen = sides.map((id) => list.find((c) => c.id === id) ?? list[0])
      running = true
      results = [null, null]
      render()
      results = await Promise.all(chosen.map((c) => runOne(copy(c.tuning), c.label)))
      running = false
      render()
    }
    const grid = el('div', '', 'ps-grid')
    results.forEach((r, i) => {
      const card = el('section', '', 'ps-result')
      const top = el('header')
      const label = r?.label ?? list.find((c) => c.id === sides[i])?.label ?? ''
      top.append(el('b', `${i === 0 ? '左' : '右'} · ${label}`))
      if (r)
        top.append(
          el('small', `${r.seconds} 秒${r.data?.model ? ` · ${r.data.model as string}` : ''}`)
        )
      card.append(top)
      if (running) card.append(el('p', '模型正在写…', 'ps-wait'))
      else if (!r) card.append(el('p', '还没跑过。', 'ps-wait'))
      else if (r.error) card.append(el('p', `没排成：${r.error}`, 'ps-error'))
      else if (r.data) {
        card.append(
          ...(target === 'story'
            ? storyResult(r.data.draft as Record<string, unknown>)
            : characterResult(r.data.fields as Record<string, unknown>))
        )
        const id = sides[i]
        const picked = list.find((c) => c.id === id)
        if (picked && id !== 'editing') {
          const adopt = button('就用这一份', () => {
            d[target] = copy(picked.tuning)
            tab = 'tune'
            status = '已换成这一份，记得保存。'
            render()
          })
          adopt.className = 'ps-adopt'
          card.append(adopt)
        }
      }
      grid.append(card)
    })
    return [input, grid]
  }

  // ---------- 画 ----------

  function renderFoot() {
    const changed = dirty()
    const note = el('p', status || (changed ? '有没保存的修改。' : ''), 'ps-status')
    note.setAttribute('role', 'status')
    const discard = button('放弃修改', () => {
      if (view) draft = copy(view.settings)
      status = ''
      render()
    })
    const keep = button(busy ? '正在保存…' : '保存', () => void save(), 'primary')
    discard.disabled = keep.disabled = busy || !changed
    foot.replaceChildren(note, discard, keep)
  }
  function render() {
    targetSeg.set(target)
    tabSeg.set(tab)
    if (!view || !draft) body.replaceChildren(el('p', status || '正在读取…', 'ps-wait'))
    else
      body.replaceChildren(...(tab === 'tune' ? tuneView(view, draft) : compareView(view, draft)))
    renderFoot()
  }

  root.showModal()
  void load()
}
