import { button, el } from '../dom'
import { newId } from '../ids'
import type { ModelOption } from '../models/session'
import { lookKey, portraitFor, templateKey } from '../portraits'
import type {
  AgentLimits,
  ClientMsg,
  JoinOutcome,
  JoinPreview,
  ModelChoice,
  PhysiologySwitches,
  PopulationView,
  ServerMsg,
} from '../protocol'
import { segmented, settingRow, switchControl } from '../settings/controls'
import type { ShellPanel } from '../shell/panel'
import type { createWorldSettings } from '../worldSettings'
import {
  applyOption,
  withField,
  type Prefab,
  type PrefabOption,
  type PrefabPersona,
  type PrefabRule,
} from '.'

// 开局设置：第一次进预制小镇（还没设世界观）时代替开局向导。玩家只改几项：人数、每位选手用的模型、
// 玩法自己的几行（预制小镇的 options：圈的位置、身份、白天长短……）、吃喝睡开不开；
// 想细调的再展开「规则」（钟点、期限、关掉哪些环节）和「人设」（这一局每位选手的名字、性格、处境，只管这一局）；
// 世界观、玩法和其余设定由预制小镇写好。
// 点「开始这一局」依次保存世界观、生理开关，定气候、布置地图上的道具、调额度（预制小镇写了才做），
// 按号码请人入住（入住时带上各自的模型；有这一局自己的人设时带上人设），换上这一局捏好的长相，最后打开玩法；
// 哪一步出错就停在那一步说明原因，再点一次从那一步接着做。做完世界还停着，玩家点左上角的 ▶ 开始。
// 跳过就交回原来的首次对话框，像普通小镇一样自己设。

type Waiter = { match: (m: ServerMsg) => unknown; resolve: (v: unknown) => void }

/** 开局做到一半时记在浏览器里的进度：刷新页面后照它接着做。只记一份，认世界编号 */
const PROGRESS_KEY = 'town.prefabSetup'
interface Progress {
  world: string
  prefab: string
  players: number
  models: (ModelChoice | null)[]
  needs: boolean
  picks: [string, string][]
  seeds: [string, number][]
  rules: [string, number | boolean][]
  personas: [number, Partial<PrefabPersona>][]
  done: Record<'worldview' | 'needs' | 'climate' | 'furnished' | 'limits' | 'dressed', boolean>
}

/** 固定种子时输入框里先填的数 */
const DEFAULT_SEED = 51

/** 室外行走空间的编号（host 编译地图时定的），开局道具按坐标放时用 */
const OUTDOOR_SPACE = 'town_outdoor'

const ALL_OFF: PhysiologySwitches = {
  fullness: false,
  energy: false,
  water: false,
  health: false,
  weather: false,
}

export function createPrefabSetup(d: {
  send: (msg: ClientMsg) => void
  worldSettings: ReturnType<typeof createWorldSettings>
  /** 能用的模型；没连模型时为空，选手先住下，连上模型后才行动 */
  modelOptions: () => ModelOption[]
  /** 做完时：请进来的居民编号（按号码） */
  onFinish: (joined: string[]) => void
}) {
  const root = el('div', '', 'guide prefab-setup')
  root.hidden = true
  root.setAttribute('role', 'dialog')
  root.setAttribute('aria-label', '开局设置')
  const card = el('section', '', 'guide-card')
  const top = el('div', '', 'guide-top')
  const title = el('b', '', 'prefab-title')
  const skip = button('跳过', () => close(), 'guide-skip')
  top.append(title, skip)
  const body = el('div', '', 'guide-body')
  const foot = el('div', '', 'guide-foot one')
  card.append(top, body, foot)
  root.append(card)
  document.body.append(root)

  let prefab: Prefab | null = null
  let players = 0
  /** 每个号码的模型；null 是没连模型、先住下 */
  let models: (ModelChoice | null)[] = []
  /** 玩法自己那几行选了什么、填的种子（按行名） */
  const picks = new Map<string, string>()
  const seeds = new Map<string, number>()
  let needs = true
  /** 「规则」里改过的项（按写进玩法设置的路径）；没改的不在里面 */
  const ruleValues = new Map<string, number | boolean>()
  /** 这一局改过的人设（按号码，从 0 数）；没改的字段照预制小镇写的 */
  const personaEdits = new Map<number, Partial<PrefabPersona>>()
  /** 「规则」「人设」展开着没有：重画以后保持原样 */
  const unfolded = new Set<string>()
  /** 村民库里的名字和头像编号（模板编号 → …），读到之前显示号码 */
  const names = new Map<string, string>()
  const keys = new Map<string, string>()
  // 做到哪一步了：出错后从这里接着做
  let running = false
  let status = ''
  let worldviewSaved = false
  let needsSaved = false
  let climateSaved = false
  let furnished = false
  let limitsSaved = false
  let dressed = false
  let modeSaved = false
  const joined: (string | null)[] = []

  /** 把选好的设置和做到哪一步记下来；浏览器不让存就算了 */
  function remember() {
    const world = d.worldSettings.state()?.world_id
    if (!prefab || !world) return
    const progress: Progress = {
      world,
      prefab: prefab.id,
      players,
      models,
      needs,
      picks: [...picks],
      seeds: [...seeds],
      rules: [...ruleValues],
      personas: [...personaEdits],
      done: {
        worldview: worldviewSaved,
        needs: needsSaved,
        climate: climateSaved,
        furnished,
        limits: limitsSaved,
        dressed,
      },
    }
    try {
      localStorage.setItem(PROGRESS_KEY, JSON.stringify(progress))
    } catch {
      // 存不了：刷新后接不上，这一页里照常能接着做
    }
  }
  function forget() {
    try {
      localStorage.removeItem(PROGRESS_KEY)
    } catch {
      // 同上
    }
  }
  /** 记着的进度是不是这个世界、这个预制小镇的 */
  function remembered(p: Prefab): Progress | null {
    try {
      const saved = JSON.parse(localStorage.getItem(PROGRESS_KEY) ?? 'null') as Progress | null
      return saved?.world === d.worldSettings.state()?.world_id && saved?.prefab === p.id
        ? saved
        : null
    } catch {
      return null
    }
  }
  /** 谁已经住进来了以镇上的人口为准：回执没等到、页面刷新过，都不会把同一个人再请一遍 */
  async function syncJoined(p: Prefab) {
    d.send({ type: 'get_population' })
    const population: PopulationView = await waitFor((m) =>
      m.type === 'population' ? m.population : undefined
    )
    for (let i = 0; i < players; i++)
      joined[i] = population.residents.find((r) => r.template_id === p.roster[i])?.id ?? null
  }
  /** 正在请入住的号码（从 0 数），头像上的号码变成橙色 */
  let joining = -1

  const waiters: Waiter[] = []
  // 等得宽一些：人多的时候画面忙，页面处理回执会慢（世界那头一步只要零点几秒）
  function waitFor<T>(match: (m: ServerMsg) => T | undefined, ms = 45000) {
    return new Promise<T>((resolve, reject) => {
      const waiter: Waiter = { match, resolve: resolve as (v: unknown) => void }
      waiters.push(waiter)
      setTimeout(() => {
        const i = waiters.indexOf(waiter)
        if (i < 0) return
        waiters.splice(i, 1)
        reject(Error('小镇没有及时回应，请检查连接后再点一次'))
      }, ms)
    })
  }
  /** 世界设定的保存：成功时接着往下走，失败时带着原因停下 */
  const saved = (submit: (done: (ok: boolean, message: string) => void) => void) =>
    new Promise<void>((resolve, reject) =>
      submit((ok, message) => (ok ? resolve() : reject(Error(message))))
    )

  function slotModel(i: number) {
    const options = d.modelOptions()
    const chosen = models[i]
    return (
      options.find((o) => o.provider === chosen?.provider && o.model === chosen.model) ??
      options[0] ??
      null
    )
  }

  /** 下拉框里一个模型的取值 */
  const choiceValue = (m: ModelChoice) => `${m.provider}\n${m.model}`
  const parseChoice = (value: string): ModelChoice => {
    const [provider, model] = value.split('\n')
    return { provider, model }
  }
  /** 模型的叫法：可选的模型一个服务一个，写服务名就分得清；有同名的服务才带上模型名 */
  const modelLabel = (o: ModelOption, options: ModelOption[]) =>
    options.filter((x) => x.name === o.name).length > 1 ? `${o.name} · ${o.model}` : o.name
  function modelSelect(options: ModelOption[]) {
    const select = el('select')
    for (const o of options) {
      const option = el('option', modelLabel(o, options))
      option.value = choiceValue(o)
      select.append(option)
    }
    return select
  }

  // 卡片从上到下：名字和一句话简介；一排选手头像（人数加减只是点亮、熄灭，卡片高度不变）；
  // 三行设置用设定页的同款控件（settings/controls.ts）。不再一组一个框，一屏放得下、不用滚动。
  function render() {
    if (!prefab) return
    const p = prefab
    title.textContent = '开局设置'
    skip.hidden = running
    const head = el('header', '', 'prefab-head')
    head.append(el('h2', p.name), el('p', p.description))
    const options = d.modelOptions()
    // 上场的人用了哪几个模型：不止一个时头像下面写各自的模型，都一样就不写（下面「模型」一行写了）
    const used = new Set(
      Array.from({ length: players }, (_, i) => slotModel(i)).map((m) => m && choiceValue(m))
    )
    const rows = el('div', '', 'set-rows prefab-settings')
    rows.append(modelRow(p, options, used), ...p.options.map(optionRow), needsRow(p))
    const extras: HTMLElement[] = []
    for (const box of [rulesFold(p), castFold(p)]) if (box) extras.push(box)
    body.replaceChildren(head, lineup(p, options, used.size > 1), rows, ...extras)

    const go = button(running ? '正在开局…' : '开始这一局', () => void start(), 'primary')
    go.disabled = running
    foot.replaceChildren(...(status ? [el('p', status, 'prefab-status')] : []), go)
  }

  /** 选手：所有号码排成两行，前 players 位亮着；模型不止一个时，点头像给这一位换模型 */
  function lineup(p: Prefab, options: ModelOption[], mixed: boolean) {
    const section = el('section', '', 'prefab-lineup')
    const bar = el('div', '', 'prefab-lineup-bar')
    const stepper = el('div', '', 'prefab-stepper')
    stepper.setAttribute('role', 'group')
    stepper.setAttribute('aria-label', '人数')
    const less = button('−', () => setPlayers(players - 1))
    const more = button('+', () => setPlayers(players + 1))
    less.disabled = running || players <= p.players.min
    more.disabled = running || players >= p.players.max
    less.setAttribute('aria-label', '少一个人')
    more.setAttribute('aria-label', '多一个人')
    stepper.append(less, el('b', `${players} 人`), more)
    bar.append(el('h3', '选手'), el('small', `${p.players.min}～${p.players.max} 人`), stepper)

    const grid = el('div', '', 'prefab-players')
    const pickable = options.length > 1
    for (let i = 0; i < p.players.max; i++) {
      const id = p.roster[i]
      const name = personaOf(p, i)?.name ?? names.get(id) ?? ''
      const on = i < players
      const cell = el('div', '', 'prefab-player')
      cell.classList.toggle('off', !on)
      cell.classList.toggle('joined', !!joined[i])
      cell.classList.toggle('joining', running && joining === i)
      const face = el('span', '', 'prefab-face')
      const look = p.cast?.[i]?.look
      const key = look ? lookKey(look) : keys.get(id)
      const url = key ? portraitFor(key) : undefined
      if (url) {
        const img = el('img')
        img.src = url
        img.alt = ''
        face.append(img)
      } else face.textContent = name.slice(0, 1)
      face.append(el('i', joined[i] ? '✓' : String(i + 1), 'prefab-no'))
      cell.append(face, el('b', name || `${i + 1} 号`))
      const model = on ? slotModel(i) : null
      // 各用各的时名字下面写用哪个模型；没上场的空着，格子一样高
      if (mixed) cell.append(el('small', model ? modelLabel(model, options) : ''))
      if (pickable && model) {
        // 整格盖一个透明下拉框：点头像就弹出统一样式的模型列表
        const select = modelSelect(options)
        select.className = 'prefab-pick'
        select.value = choiceValue(model)
        select.disabled = running || joined[i] !== undefined
        select.setAttribute('aria-label', `${name || `${i + 1} 号`}用的模型`)
        select.onchange = () => {
          models[i] = parseChoice(select.value)
          render()
        }
        cell.append(select)
      }
      grid.append(cell)
    }
    section.append(bar, grid)
    return section
  }

  /** 模型：只有一个时写出来；多个时下拉框一次给所有人换，各用各的时框里写「各用各的」 */
  function modelRow(p: Prefab, options: ModelOption[], used: Set<string | null>) {
    if (!options.length)
      return settingRow(
        '模型',
        '还没连接模型：选手先住下，连上后才会行动',
        el('span', '未连接', 'ui-chip')
      )
    if (options.length === 1)
      return settingRow(
        '模型',
        '所有选手都用它',
        el('span', `${options[0].name} · ${options[0].model}`, 'prefab-model')
      )
    const select = modelSelect(options)
    if (used.size > 1) {
      const mixed = el('option', '各用各的')
      mixed.value = ''
      mixed.hidden = true
      select.prepend(mixed)
      select.value = ''
    } else select.value = [...used][0] ?? choiceValue(options[0])
    select.disabled = running
    select.setAttribute('aria-label', '所有选手用的模型')
    select.onchange = () => {
      const choice = parseChoice(select.value)
      // 已经住进来的人模型定了，不跟着换
      models = Array.from({ length: p.players.max }, (_, i) =>
        joined[i] ? (models[i] ?? slotModel(i)) : choice
      )
      render()
    }
    return settingRow('模型', '点头像可以给某一位单独换', select)
  }

  /** 玩法自己的一行：分段选择；选到要种子的取值时下面多一个种子输入框 */
  function optionRow(o: PrefabOption) {
    const picked = picks.get(o.label) ?? o.initial
    const seg = segmented(
      o.label,
      o.choices.map(({ value, label }) => ({ value, label })),
      (value) => {
        if (picked === value) return
        picks.set(o.label, value)
        render()
        if (value === o.seed) body.querySelector<HTMLInputElement>('.prefab-seed')?.select()
      }
    )
    seg.set(picked)
    seg.el.querySelectorAll('button').forEach((b) => (b.disabled = running))
    const hint = o.choices.find((c) => c.value === picked)?.hint ?? ''
    const row = settingRow(o.label, hint, seg.el)
    if (o.seed && picked === o.seed) {
      const line = el('label', '', 'set-row-desc prefab-seed-line')
      const input = el('input', '', 'prefab-seed')
      input.type = 'number'
      input.min = '1'
      input.max = '4294967295'
      input.value = String(seeds.get(o.label) ?? DEFAULT_SEED)
      input.disabled = running
      input.onchange = () => {
        const n = Math.floor(Number(input.value))
        if (Number.isFinite(n) && n >= 1 && n <= 4294967295) seeds.set(o.label, n)
        input.value = String(seeds.get(o.label) ?? DEFAULT_SEED)
      }
      line.append('种子', input)
      row.querySelector('.set-row-text')?.append(line)
    }
    return row
  }

  function needsRow(p: Prefab) {
    const toggle = switchControl('吃喝睡', (on) => {
      needs = on
      render()
    })
    toggle.set(needs)
    toggle.el.disabled = running || needsSaved
    return settingRow('吃喝睡', needs ? p.needsHint.on : p.needsHint.off, toggle.el)
  }

  /** 这一位选手这一局的人设：预制小镇写的，盖上玩家在「人设」里改的 */
  function personaOf(p: Prefab, i: number): PrefabPersona | undefined {
    const base = p.cast?.[i]
    return base ? { ...base, ...personaEdits.get(i) } : undefined
  }

  /** 收起来的一栏；展开没展开记在 unfolded 里 */
  function fold(key: string, label: string, note: string) {
    const box = el('details', '', 'prefab-fold')
    box.open = unfolded.has(key)
    box.ontoggle = () => (box.open ? unfolded.add(key) : unfolded.delete(key))
    const summary = el('summary')
    summary.append(el('b', label), el('small', note))
    box.append(summary)
    return box
  }

  const clockOf = (minute: number) =>
    `${String(Math.floor(minute / 60)).padStart(2, '0')}:${String(minute % 60).padStart(2, '0')}`

  /** 「规则」：钟点、期限和可以关掉的环节。只记改过的项，「恢复默认」全部清掉 */
  function rulesFold(p: Prefab) {
    if (!p.rules?.length) return null
    const locked = running || modeSaved
    const box = fold(
      'rules',
      '规则',
      ruleValues.size ? `改了 ${ruleValues.size} 项` : '钟点、期限和环节都照原样'
    )
    const rows = el('div', '', 'set-rows prefab-settings')
    const change = (r: PrefabRule, value: number | boolean) => {
      if (value === r.initial) ruleValues.delete(r.field)
      else ruleValues.set(r.field, value)
      render()
    }
    for (const r of p.rules) {
      const value = ruleValues.get(r.field) ?? r.initial
      if (r.kind === 'switch') {
        const toggle = switchControl(r.label, (on) => change(r, on))
        toggle.set(value === true)
        toggle.el.disabled = locked
        rows.append(settingRow(r.label, r.hint, toggle.el))
        continue
      }
      const input = el('input', '', 'prefab-rule')
      input.disabled = locked
      input.setAttribute('aria-label', r.label)
      const min = r.min ?? 0
      const max = r.max ?? 1439
      if (r.kind === 'time') {
        input.type = 'time'
        input.value = clockOf(Number(value))
        input.onchange = () => {
          const [h, m] = input.value.split(':').map(Number)
          const minute = h * 60 + m
          if (Number.isFinite(minute)) change(r, Math.min(max, Math.max(min, minute)))
          else render()
        }
      } else {
        input.type = 'number'
        input.min = String(min)
        input.max = String(max)
        input.value = String(value)
        input.onchange = () => {
          const n = Math.floor(Number(input.value))
          if (Number.isFinite(n)) change(r, Math.min(max, Math.max(min, n)))
          else render()
        }
      }
      const control = el('label', '', 'prefab-rule-line')
      control.append(input, ...(r.kind === 'minutes' ? ['分钟'] : []))
      rows.append(settingRow(r.label, r.hint, control))
    }
    const reset = button('恢复默认', () => {
      ruleValues.clear()
      render()
    })
    reset.disabled = locked || !ruleValues.size
    box.append(rows, reset)
    return box
  }

  /** 「人设」：这一局每位选手的名字和几段人设，改了只管这一局；已经住进来的不能再改 */
  function castFold(p: Prefab) {
    if (!p.cast?.length) return null
    const box = fold(
      'cast',
      '人设',
      personaEdits.size ? `改了 ${personaEdits.size} 位` : '每位选手的名字、性格和处境'
    )
    const fields: { key: keyof PrefabPersona; label: string; long?: boolean }[] = [
      { key: 'name', label: '名字' },
      { key: 'intro', label: '一句话简介' },
      { key: 'personality', label: '性格', long: true },
      { key: 'situation', label: '今天的处境', long: true },
      { key: 'drives', label: '想要什么', long: true },
    ]
    for (let i = 0; i < players; i++) {
      const base = p.cast[i]
      if (!base) continue
      const now = personaOf(p, i) ?? base
      const locked = running || joined[i] !== undefined
      const one = fold(`cast-${i}`, `${i + 1} 号 ${now.name}`, personaEdits.has(i) ? '改过' : '')
      one.classList.add('prefab-persona')
      for (const f of fields) {
        const input = f.long ? el('textarea') : el('input')
        input.value = String(now[f.key] ?? '')
        input.disabled = locked
        input.onchange = () => {
          const text = input.value.trim()
          const edits = { ...personaEdits.get(i) }
          // 名字不能空；改回原样就不算改过
          if (text === String(base[f.key] ?? '') || (f.key === 'name' && !text)) delete edits[f.key]
          else Object.assign(edits, { [f.key]: text })
          if (Object.keys(edits).length) personaEdits.set(i, edits)
          else personaEdits.delete(i)
          render()
        }
        const line = el('label', '', 'prefab-field')
        line.append(el('span', f.label), input)
        one.append(line)
      }
      box.append(one)
    }
    return box
  }

  function setPlayers(n: number) {
    if (!prefab) return
    players = Math.min(prefab.players.max, Math.max(prefab.players.min, n))
    render()
  }

  /** 改道具的一条命令：等回执，没成时带着原因停下 */
  async function propCommand(msg: (world_id: string, request_id: string) => ClientMsg) {
    const world = d.worldSettings.state()
    if (!world) throw Error('还没读到世界设定')
    const request_id = newId()
    d.send(msg(world.world_id, request_id))
    const reply = await waitFor((m) =>
      m.type === 'prop_result' && m.request_id === request_id ? m : undefined
    )
    if (!reply.ok) throw Error(reply.message)
  }

  /** 按预制小镇的布置撤掉、放上道具；撤和放同一个编号再做一次都没有副作用，出错后可以从头再来 */
  async function furnish(f: NonNullable<Prefab['furnish']>) {
    for (const id of f.remove)
      // 建镇生成器换了版本、编号对不上时少撤一件不要紧
      await propCommand((world_id, request_id) => ({
        type: 'remove_prop',
        request_id,
        world_id,
        id,
      })).catch(() => undefined)
    for (const { id, definition, locale, near } of f.place)
      await propCommand((world_id, request_id) => ({
        type: 'place_prop',
        request_id,
        world_id,
        instance: { id, definition, locale },
        ...(near && { near: { space: OUTDOOR_SPACE, position: near } }),
      }))
  }

  /** 额度：读到现在的，把预制小镇写了的几项换掉再存 */
  async function setLimits(limits: Partial<AgentLimits>) {
    d.send({ type: 'get_agent_limits' })
    const current: AgentLimits = await waitFor((m) =>
      m.type === 'agent_limits' ? m.limits : undefined
    )
    const request_id = newId()
    d.send({ type: 'set_agent_limits', request_id, limits: { ...current, ...limits } })
    const reply = await waitFor((m) =>
      m.type === 'config_saved' && m.request_id === request_id ? m : undefined
    )
    if (!reply.ok) throw Error(reply.message)
  }

  async function join(
    templateId: string,
    model: ModelChoice | null,
    persona?: PrefabPersona
  ): Promise<string> {
    d.send({
      type: 'preview_resident_join',
      template_id: templateId,
      ...(persona ? { name: persona.name } : {}),
    })
    const preview: JoinPreview = await waitFor((m) =>
      m.type === 'join_preview' && m.preview.template_id === templateId ? m.preview : undefined
    )
    if (!preview.ok) throw Error(preview.problems.join('；') || '现在不能加入')
    const world = d.worldSettings.state()
    if (!world) throw Error('还没读到世界设定')
    const op_id = newId()
    d.send({
      type: 'join_resident',
      op_id,
      world_id: world.world_id,
      template_id: templateId,
      template_version: preview.template_version,
      name: preview.name,
      home: preview.home,
      ...(model ? { model } : {}),
      ...(persona ? { persona: personaFields(persona) } : {}),
    })
    const outcome: JoinOutcome = await waitFor((m) =>
      m.type === 'resident_joined' && m.outcome.op_id === op_id
        ? m.outcome
        : m.type === 'join_result' && m.op_id === op_id
          ? (m.outcome ?? undefined)
          : undefined
    )
    if (!outcome.ok || !outcome.resident_id) throw Error(outcome.message)
    return outcome.resident_id
  }

  /** 入住带的人设：捏好的长相不在人设里，入住后另外换上 */
  function personaFields({ look: _look, ...fields }: PrefabPersona) {
    return fields
  }

  /** 选手入住后换上这一局捏好的长相：一位一位换，每次带上刚回来的人口版本 */
  async function dress(p: Prefab) {
    d.send({ type: 'get_population' })
    let population: PopulationView = await waitFor((m) =>
      m.type === 'population' ? m.population : undefined
    )
    for (let i = 0; i < players; i++) {
      const look = p.cast?.[i]?.look
      const registration = population.residents.find((r) => r.id === joined[i])
      if (!look || !registration) continue
      if (JSON.stringify(registration.appearance.look) === JSON.stringify(look)) continue
      const request_id = newId()
      d.send({
        type: 'set_resident_appearance',
        request_id,
        world_id: population.world_id,
        expected_revision: population.revision,
        resident_id: registration.id,
        appearance: { ...registration.appearance, look },
      })
      const reply = await waitFor((m) =>
        m.type === 'resident_appearance_saved' && m.request_id === request_id ? m : undefined
      )
      if (!reply.ok) throw Error(reply.message)
      population = reply.population
    }
  }

  async function start() {
    const p = prefab
    if (!p || running) return
    running = true
    status = ''
    render()
    const ws = d.worldSettings
    remember()
    try {
      await syncJoined(p)
      if (!worldviewSaved) {
        status = '正在写世界观…'
        render()
        await saved((done) => ws.saveWorldview(p.worldview, done))
        worldviewSaved = true
        remember()
      }
      if (!needsSaved) {
        status = '正在设定吃喝睡…'
        render()
        const revision = ws.state()?.revision
        if (revision === undefined) throw Error('还没读到世界设定')
        await saved((done) => ws.savePhysiology(needs ? p.needs : ALL_OFF, revision, done))
        needsSaved = true
        remember()
      }
      if (p.climate && !climateSaved) {
        status = '正在定气候…'
        render()
        const climate = p.climate
        await saved((done) => ws.saveClimate(climate, done))
        climateSaved = true
        remember()
      }
      if (p.furnish && !furnished) {
        status = '正在布置地图上的东西…'
        render()
        await furnish(p.furnish)
        furnished = true
        remember()
      }
      if (p.limits && !limitsSaved) {
        status = '正在调额度…'
        render()
        await setLimits(p.limits)
        limitsSaved = true
        remember()
      }
      for (let i = 0; i < players; i++) {
        if (joined[i]) continue
        status = `正在请 ${i + 1} 号入住…`
        joining = i
        render()
        joined[i] = await join(p.roster[i], slotModel(i), personaOf(p, i))
        joining = -1
      }
      if (p.cast?.some((c) => c.look) && !dressed) {
        status = '正在换上这一局的长相…'
        render()
        await dress(p)
        dressed = true
        remember()
      }
      if (!modeSaved) {
        status = '正在打开玩法…'
        render()
        const picked = p.options.reduce((c, o) => {
          const value = picks.get(o.label) ?? o.initial
          const seed = o.seed && value === o.seed ? (seeds.get(o.label) ?? DEFAULT_SEED) : null
          return applyOption(c, o, value, seed)
        }, p.gameMode)
        // 「规则」里改过的项；一项没改时玩法设置里没有这一栏
        const choice = [...ruleValues].reduce(
          (c, [field, value]) => withField(c, field, value),
          picked
        )
        await saved((done) => ws.saveGameMode(choice, done))
        modeSaved = true
      }
      forget()
      running = false
      root.hidden = true
      d.onFinish(joined.filter((id): id is string => !!id))
    } catch (error) {
      running = false
      joining = -1
      status = `${status.replace(/…$/, '')}时出错：${error instanceof Error ? error.message : String(error)}。再点一次「开始这一局」从这一步接着做。`
      render()
    }
  }

  function close() {
    if (running) return
    forget()
    root.hidden = true
    // 跳过：交回首次对话框，像普通小镇一样自己设
    d.worldSettings.setFirstRun(null)
  }

  const panel: ShellPanel = {
    handleMsg(m: ServerMsg) {
      if (m.type === 'resident_library') {
        for (const t of m.templates) {
          names.set(t.template_id, t.name)
          keys.set(t.template_id, templateKey(t))
        }
        if (!root.hidden) render()
      }
      for (const w of [...waiters]) {
        const value = w.match(m)
        if (value !== undefined) {
          waiters.splice(waiters.indexOf(w), 1)
          w.resolve(value)
        }
      }
    },
  }

  return {
    panel,
    /** 打开这个预制小镇的开局设置 */
    open(p: Prefab) {
      prefab = p
      players = p.players.default
      models = []
      needs = p.needsDefault
      ruleValues.clear()
      personaEdits.clear()
      unfolded.clear()
      root.hidden = false
      d.send({ type: 'get_resident_library', filter: { kind: 'builtin' } })
      render()
    },
    /** 上次开局做到一半（世界观已经写了、玩法还没打开）页面就没了：照记着的设置和进度打开，点一下接着做。
     *  没有记着这个世界的进度就不打开，返回 false */
    resume(p: Prefab) {
      const saved = remembered(p)
      if (!saved || !root.hidden) return false
      prefab = p
      players = Math.min(Math.max(saved.players, p.players.min), p.players.max)
      models = saved.models
      needs = saved.needs
      picks.clear()
      seeds.clear()
      ruleValues.clear()
      personaEdits.clear()
      for (const [k, v] of saved.picks) picks.set(k, v)
      for (const [k, v] of saved.seeds) seeds.set(k, v)
      for (const [k, v] of saved.rules) ruleValues.set(k, v)
      for (const [k, v] of saved.personas) personaEdits.set(k, v)
      worldviewSaved = saved.done.worldview
      needsSaved = saved.done.needs
      climateSaved = saved.done.climate
      furnished = saved.done.furnished
      limitsSaved = saved.done.limits
      dressed = saved.done.dressed
      status = '上次开局做到一半：点「开始这一局」从停下的那一步接着做。'
      root.hidden = false
      d.send({ type: 'get_resident_library', filter: { kind: 'builtin' } })
      render()
      return true
    },
    /** 模型配置变了（连上了新模型）：重画选择框 */
    refresh() {
      if (!root.hidden && !running) render()
    },
  }
}
