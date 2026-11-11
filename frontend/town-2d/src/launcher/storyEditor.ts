import { button, el } from 'town-shared/dom'
import { errorText } from 'town-shared/format'
import { newId } from 'town-shared/ids'
import type { StorylineBeat, StorylineMilestone, StorylineWhen } from 'town-shared/protocol'
import { specOf, type StoryCharacter, type StoryDraft } from 'town-shared/prefabs/scenario'
import { plannerKinds } from '../../../../scripts/mapgen/plan.mjs'
import library from '../../../../data/mapgen/templates.json'
import { STYLE_PRESETS } from '../map/themes/presets'
import { api } from './api'
import { creationRequest } from './localCreation'
import './storyEditor.css'

// 故事编辑器：贴一段故事或提纲，连着的模型把它排成一整套——角色、地点和风格、到点发生的节拍、要追踪的里程碑。
// 两步：先写提纲、定角色数和天数，点「让模型编排」（管理服务的 /story 调一次模型）；
// 再在「过目」里逐项改——每个字段都能改，节拍和里程碑能加能删——点「建成小镇」：
// 草稿先送回 /story 理一遍、查一遍（和开局时内核查的是同一套），再按草稿里的地点生成地图、存成小镇，
// 草稿跟着小镇存进建镇说明。进镇时开局设置按草稿走（town-shared/prefabs/scenario.ts），打开的是故事模式。

/** 模型能用的地点种类（不含住处：每个角色的住处自动安排）和风格 */
const KINDS: { kind: string; name: string }[] = plannerKinds(library.templates)
  .filter((k: { kind: string }) => k.kind !== 'home')
  .map(({ kind, name }: { kind: string; name: string }) => ({ kind, name }))
const PRESETS = STYLE_PRESETS.map(({ id, name }) => ({ id, name }))
const OUTLINE_MAX = 6000

const clockOf = (minute: number) =>
  `${String(Math.floor(minute / 60)).padStart(2, '0')}:${String(minute % 60).padStart(2, '0')}`
const minuteOf = (clock: string) => {
  const [h, m] = clock.split(':').map(Number)
  return Number.isFinite(h) && Number.isFinite(m) ? h * 60 + m : 0
}
/** 「1, 2，3」→ [1, 2, 3] */
const seatsOf = (text: string) =>
  text
    .split(/[^0-9]+/)
    .filter(Boolean)
    .map(Number)

export function initStoryEditor(d: {
  /** 小镇建好了：回选择小镇页选中它 */
  onSaved: (id: string) => void | Promise<void>
  /** 首页设定里的连接模型弹层，返回 'connected' 表示连上了 */
  connectModel?: () => Promise<string>
}) {
  const open = document.createElement('button')
  open.className = 'create-town-button story-town-button'
  open.append(el('span', '✎', 'story-town-icon'), '写一个故事')
  document.querySelector('[data-create-town-slot]')?.append(open)

  const panel = el('section', '', 'story-editor')
  panel.hidden = true
  panel.setAttribute('role', 'dialog')
  panel.setAttribute('aria-label', '写一个故事')
  const head = el('header')
  const body = el('div', '', 'story-body')
  const foot = el('footer')
  panel.append(head, body, foot)
  document.body.append(panel)

  let outline = ''
  let people = 6
  let days = 1
  let draft: StoryDraft | null = null
  let busy = false
  let status = ''
  let model: { available: boolean; provider?: string; model?: string } | null = null
  let abort: AbortController | null = null

  async function askModel() {
    try {
      model = (await api<{ model: typeof model }>('/options')).model
    } catch {
      model = null
    }
    render()
  }

  // ---------- 控件 ----------

  /** 一行：标签和输入框；改了就写回去，不整页重画（免得丢焦点） */
  function field(
    label: string,
    value: string,
    set: (v: string) => void,
    o: { long?: boolean; type?: string; hint?: string } = {}
  ) {
    const input = o.long ? el('textarea') : el('input')
    if (!o.long && o.type) (input as HTMLInputElement).type = o.type
    input.value = value
    input.disabled = busy
    input.oninput = () => set(input.value)
    const line = el('label', '', o.long ? 'story-field long' : 'story-field')
    line.append(el('span', label), input)
    if (o.hint) line.append(el('small', o.hint))
    return line
  }
  function pick(
    label: string,
    value: string,
    options: { value: string; label: string }[],
    set: (v: string) => void
  ) {
    const select = el('select')
    for (const o of options) {
      const option = el('option', o.label)
      option.value = o.value
      select.append(option)
    }
    select.value = value
    select.disabled = busy
    select.onchange = () => {
      set(select.value)
      render()
    }
    const line = el('label', '', 'story-field')
    line.append(el('span', label), select)
    return line
  }
  function check(label: string, value: boolean, set: (v: boolean) => void) {
    const input = el('input')
    input.type = 'checkbox'
    input.checked = value
    input.disabled = busy
    input.onchange = () => set(input.checked)
    const line = el('label', '', 'story-check')
    line.append(input, label)
    return line
  }
  /** 一组：标题、每一项一张卡（带删除）、加一项 */
  function group<T>(
    title: string,
    items: T[],
    card: (item: T, i: number) => HTMLElement[],
    name: (item: T, i: number) => string,
    add?: () => T
  ) {
    const box = el('section', '', 'story-group')
    box.append(el('h3', `${title}（${items.length}）`))
    items.forEach((item, i) => {
      const one = el('details', '', 'story-card')
      const summary = el('summary', name(item, i))
      const remove = button('删掉', () => {
        items.splice(i, 1)
        render()
      })
      remove.disabled = busy
      one.append(summary, ...card(item, i), remove)
      box.append(one)
    })
    if (add) {
      const more = button(`加一个${title}`, () => {
        items.push(add())
        render()
      })
      more.disabled = busy
      box.append(more)
    }
    return box
  }

  // ---------- 第一步：提纲 ----------

  function outlineStep() {
    const text = el('textarea', '', 'story-outline')
    text.placeholder =
      '贴一段故事、一个提纲，或者只写一句设定。比如：海边小镇的老灯塔要卖了，守塔人不肯，镇长欠着一笔说不清的债……'
    text.maxLength = OUTLINE_MAX
    text.value = outline
    text.disabled = busy
    const count = el('small', `${outline.length}/${OUTLINE_MAX}`)
    text.oninput = () => {
      outline = text.value
      count.textContent = `${outline.length}/${OUTLINE_MAX}`
      go.disabled = busy || !model?.available || !outline.trim()
    }
    const options = el('div', '', 'story-row')
    options.append(
      pick(
        '角色',
        String(people),
        Array.from({ length: 11 }, (_, i) => ({ value: String(i + 2), label: `${i + 2} 个` })),
        (v) => (people = Number(v))
      ),
      pick(
        '历时',
        String(days),
        [1, 2, 3].map((n) => ({ value: String(n), label: `${n} 天` })),
        (v) => (days = Number(v))
      )
    )
    const line = el('p', '', 'story-model')
    if (!model) line.textContent = '正在问模型能不能用…'
    else if (model.available)
      line.textContent = `用 ${model.provider} · ${model.model} 编排，点一次调用一次。`
    else {
      line.append('还没连接模型：连上一个模型服务，才能让它编排故事。')
      if (d.connectModel)
        line.append(
          button('连接模型', async () => {
            if ((await d.connectModel?.()) === 'connected') await askModel()
          })
        )
    }
    const go = button(busy ? '模型正在编排…' : '让模型编排', () => void generate(), 'primary')
    go.disabled = busy || !model?.available || !outline.trim()
    body.replaceChildren(text, count, options, line)
    foot.replaceChildren(
      ...(status ? [el('p', status, 'story-status')] : []),
      ...(draft ? [button('回到上一版草稿', () => show('review'))] : []),
      go
    )
  }

  async function generate() {
    busy = true
    status = '模型正在把故事排成角色、节拍和里程碑，可能要一两分钟…'
    render()
    abort = new AbortController()
    try {
      const reply = await api<{ draft: StoryDraft }>(
        '/story',
        { outline, cast: people, days, kinds: KINDS, presets: PRESETS },
        AbortSignal.any([abort.signal, AbortSignal.timeout(300000)])
      )
      draft = reply.draft
      status = ''
      step = 'review'
    } catch (error) {
      status = abort.signal.aborted ? '' : `没排成：${errorText(error)}`
    }
    busy = false
    render()
  }

  // ---------- 第二步：过目 ----------

  function castCard(c: StoryCharacter) {
    return [
      field('名字', c.name, (v) => (c.name = v)),
      field('在故事里是谁（公开）', c.role, (v) => (c.role = v)),
      field('想达成什么（只有本人知道）', c.goal, (v) => (c.goal = v), { long: true }),
      field('秘密（没有就空着）', c.secret ?? '', (v) => (c.secret = v || undefined), {
        long: true,
      }),
      field('一句话简介', c.intro, (v) => (c.intro = v)),
      field('性格', c.personality, (v) => (c.personality = v), { long: true }),
      field('说话方式', c.speech, (v) => (c.speech = v)),
      field('经历', c.background, (v) => (c.background = v), { long: true }),
    ]
  }

  function placeOptions(draft: StoryDraft) {
    return [
      { value: '', label: '（不指定）' },
      ...draft.places.map((p) => ({ value: p.name, label: p.name })),
    ]
  }

  function beatCard(draft: StoryDraft, b: StorylineBeat) {
    const scene = b.scene
    const row = el('div', '', 'story-row')
    row.append(
      pick(
        '第几天',
        String(b.day),
        Array.from({ length: 14 }, (_, i) => ({ value: String(i + 1), label: `第 ${i + 1} 天` })),
        (v) => (b.day = Number(v))
      ),
      field('时刻', clockOf(b.minute), (v) => (b.minute = minuteOf(v)), { type: 'time' }),
      pick('地点', b.place ?? '', placeOptions(draft), (v) => (b.place = v || undefined))
    )
    return [
      field('节拍名', b.title, (v) => (b.title = v)),
      row,
      field('所有人听到的', b.announce, (v) => (b.announce = v), { long: true }),
      field('和谁有关', (b.who ?? []).join('、'), (v) => (b.who = seatsOf(v)), {
        hint: '座号，空着是所有人；这些人会被叫醒',
      }),
      field('只对他们说的', b.brief ?? '', (v) => (b.brief = v || undefined), { long: true }),
      field(
        '轮流说话的戏',
        (scene?.speakers ?? []).join('、'),
        (v) => {
          const speakers = seatsOf(v)
          b.scene = speakers.length
            ? { speakers, minutes: scene?.minutes ?? 15, chars: scene?.chars ?? 300 }
            : undefined
        },
        { hint: '按次序填座号，每人说一段；空着就没有这场戏' }
      ),
    ]
  }

  function milestoneCard(draft: StoryDraft, m: StorylineMilestone) {
    const w = m.when
    const seats = (current: number[] | undefined, set: (s: number[] | undefined) => void) =>
      field(
        '谁',
        (current ?? []).join('、'),
        (v) => {
          const list = seatsOf(v)
          set(list.length ? list : undefined)
        },
        { hint: '座号，空着是所有人' }
      )
    const how: HTMLElement[] = []
    if (w.kind === 'beat')
      how.push(
        pick(
          '哪个节拍',
          w.beat,
          draft.beats.map((b) => ({ value: b.id, label: b.title })),
          (v) => (w.beat = v)
        )
      )
    else if (w.kind === 'gather')
      how.push(
        pick('在哪里', w.place, placeOptions(draft).slice(1), (v) => (w.place = v)),
        seats(w.seats, (s) => (w.seats = s)),
        field('至少几个人', String(w.count), (v) => (w.count = Math.max(1, Number(v) || 1)), {
          type: 'number',
        })
      )
    else
      how.push(
        seats(w.seats, (s) => (w.seats = s)),
        check('只有能宣布的人知道有这一项', m.secret ?? false, (v) => (m.secret = v || undefined))
      )
    return [
      field('里程碑名', m.title, (v) => (m.title = v)),
      field('什么算达成', m.detail, (v) => (m.detail = v)),
      pick(
        '怎么算',
        w.kind,
        [
          { value: 'beat', label: '某个节拍发生了' },
          { value: 'gather', label: '几个人同时到了某个地点' },
          { value: 'declare', label: '由当事人宣布' },
        ],
        (v) => {
          const next: StorylineWhen =
            v === 'beat'
              ? { kind: 'beat', beat: draft.beats[0]?.id ?? '' }
              : v === 'gather'
                ? { kind: 'gather', place: draft.places[0]?.name ?? '', count: 2 }
                : { kind: 'declare' }
          m.when = next
          if (v !== 'declare') m.secret = undefined
        }
      ),
      ...how,
    ]
  }

  function reviewStep(draft: StoryDraft) {
    const basics = el('section', '', 'story-group')
    const row = el('div', '', 'story-row')
    row.append(
      pick(
        '风格',
        draft.style.preset,
        PRESETS.map((p) => ({ value: p.id, label: p.name })),
        (v) => (draft.style.preset = v)
      ),
      check('会饿会渴会困', draft.style.needs, (v) => (draft.style.needs = v))
    )
    const end = el('div', '', 'story-row')
    end.append(
      pick(
        '收场在',
        String(draft.ends_day),
        Array.from({ length: 14 }, (_, i) => ({ value: String(i + 1), label: `第 ${i + 1} 天` })),
        (v) => (draft.ends_day = Number(v))
      ),
      field('时刻', clockOf(draft.ends_minute), (v) => (draft.ends_minute = minuteOf(v)), {
        type: 'time',
      })
    )
    basics.append(
      el('h3', '故事'),
      field('故事名', draft.title, (v) => (draft.title = v)),
      field('前情（所有角色都知道）', draft.premise, (v) => (draft.premise = v), { long: true }),
      field('小镇是什么样的地方', draft.worldview, (v) => (draft.worldview = v), { long: true }),
      row,
      end,
      field('收场白', draft.epilogue ?? '', (v) => (draft.epilogue = v || undefined))
    )
    const used = new Set(draft.beats.map((b) => b.id))
    const fresh = (prefix: string, taken: Set<string>) => {
      for (let n = taken.size + 1; ; n++) if (!taken.has(`${prefix}-${n}`)) return `${prefix}-${n}`
    }
    body.replaceChildren(
      basics,
      group('角色', draft.cast, castCard, (c, i) => `${i + 1} 号 ${c.name} · ${c.role}`),
      group(
        '地点',
        draft.places,
        (p) => [
          pick(
            '种类',
            p.kind,
            KINDS.map((k) => ({ value: k.kind, label: k.name })),
            (v) => (p.kind = v)
          ),
          field('名字', p.name, (v) => (p.name = v)),
        ],
        (p) => p.name,
        () => ({ kind: KINDS[0].kind, name: `新地点${draft.places.length + 1}` })
      ),
      group(
        '节拍',
        draft.beats,
        (b) => beatCard(draft, b),
        (b) => `${draft.ends_day > 1 ? `第 ${b.day} 天 ` : ''}${clockOf(b.minute)} ${b.title}`,
        () => ({
          id: fresh('beat', used),
          title: '新的一幕',
          day: draft.beats.at(-1)?.day ?? 1,
          minute: Math.min(1439, (draft.beats.at(-1)?.minute ?? 480) + 60),
          announce: '发生了一件事。',
        })
      ),
      group(
        '里程碑',
        draft.milestones,
        (m) => milestoneCard(draft, m),
        (m) => m.title,
        () => ({
          id: fresh('mark', new Set(draft.milestones.map((m) => m.id))),
          title: '新的里程碑',
          detail: '什么算达成',
          when: { kind: 'declare' } satisfies StorylineWhen,
        })
      )
    )
    const build = button(busy ? '正在建镇…' : '建成小镇', () => void buildTown(draft), 'primary')
    build.disabled = busy
    const back = button('改提纲重排', () => show('outline'))
    back.disabled = busy
    foot.replaceChildren(...(status ? [el('p', status, 'story-status')] : []), back, build)
  }

  /** 草稿理一遍、查一遍，按它的地点生成地图，存成小镇（草稿跟着存进建镇说明） */
  async function buildTown(current: StoryDraft) {
    busy = true
    status = '正在检查故事…'
    render()
    try {
      const { draft: tidy } = await api<{ draft: StoryDraft }>('/story', {
        draft: current,
        kinds: KINDS,
        presets: PRESETS,
      })
      draft = tidy
      status = '正在按故事里的地点生成小镇…'
      render()
      const request = creationRequest()
      const seed = 1 + Math.floor(Math.random() * 999_999)
      const preview = await request<{ id: string; version: string }>('/preview', {
        request_id: newId(),
        spec: specOf(tidy, seed),
      })
      const saved = await request<{ id: string }>('/save', {
        draft_id: preview.id,
        expected_version: preview.version,
        request_id: newId(),
        name: [...tidy.title].slice(0, 16).join(''),
        creation: {
          description: tidy.premise,
          worldview: tidy.worldview,
          residents: [],
          scenario: tidy,
        },
      })
      busy = false
      status = ''
      close()
      await d.onSaved(saved.id)
    } catch (error) {
      busy = false
      status = `没建成：${errorText(error)}`
      render()
    }
  }

  // ---------- 外壳 ----------

  let step: 'outline' | 'review' = 'outline'
  function show(next: typeof step) {
    step = next
    status = ''
    render()
  }
  function render() {
    const title = step === 'outline' ? '写一个故事' : '过目：模型排好的故事'
    const hint =
      step === 'outline'
        ? '模型把它排成角色、地点、到点发生的事和要追踪的里程碑；角色自己拿主意，故事可以走偏。'
        : '每一项都能改。节拍到点宣布、叫醒有关的人；里程碑只是记账，不强迫谁。'
    head.replaceChildren(
      el('h2', title),
      el('p', hint),
      button('关闭', () => close(), 'story-close')
    )
    if (step === 'review' && draft) reviewStep(draft)
    else outlineStep()
  }
  function close() {
    abort?.abort()
    busy = false
    panel.hidden = true
  }
  open.onclick = () => {
    panel.hidden = false
    status = ''
    render()
    void askModel()
  }
}
