import type { StorylineBeat, StorylineMilestone, StorylineScript } from '../protocol'
import type { Prefab, PrefabPersona } from '.'

// 故事草稿：故事编辑器（选择小镇页的「写一个故事」）让模型按一段故事或提纲写出来的整套东西——
// 角色（人设加上在故事里的身份、目标、秘密）、地点和风格、节拍、里程碑、收场。
// 草稿跟着小镇存在建镇说明里（creation.scenario）；这里把它变成建镇参数和一个预制小镇，
// 进镇时和手写的预制小镇走同一套开局设置。草稿合不合规由管理服务的 /story 查（crates/agent/src/story.rs）。

export interface StoryCharacter {
  name: string
  age: string
  gender: string
  occupation: string
  intro: string
  tags: string[]
  personality: string
  speech: string
  background: string
  drives: string
  /** 在故事里的身份（公开） */
  role: string
  /** 这个故事里想达成什么（只有本人知道） */
  goal: string
  secret?: string
}

export interface StoryDraft {
  title: string
  premise: string
  worldview: string
  style: { preset: string; needs: boolean }
  /** 住处以外的地点；每个角色的住处自动安排 */
  places: { kind: string; name: string }[]
  cast: StoryCharacter[]
  beats: StorylineBeat[]
  milestones: StorylineMilestone[]
  ends_day: number
  ends_minute: number
  epilogue?: string
}

/** 借来入住的村民库模板（只借名额和长相，人设用草稿里写的），最多 12 位 */
const ROSTER = [
  'builtin_ashu',
  'builtin_xiaohe',
  'builtin_finn',
  'builtin_luna',
  'builtin_stone',
  'builtin_mira',
  'builtin_noah',
  'builtin_tan',
  'builtin_alan',
  'builtin_iris',
  'builtin_june',
  'builtin_reed',
]

/** 草稿里剧本那一半：开故事模式时带的 */
export function scriptOf(draft: StoryDraft): StorylineScript {
  return {
    title: draft.title,
    premise: draft.premise,
    cast: draft.cast.map(({ role, goal, secret }) => ({ role, goal, ...(secret && { secret }) })),
    beats: draft.beats,
    milestones: draft.milestones,
    ends_day: draft.ends_day,
    ends_minute: draft.ends_minute,
    ...(draft.epilogue && { epilogue: draft.epilogue }),
  }
}

/** 建镇参数：每个角色一间住处，加上草稿里的地点；地点和住处都按故事里的名字起名 */
export function specOf(draft: StoryDraft, seed: number): Record<string, unknown> {
  const counts = new Map<string, number>()
  const names: Record<string, string> = {}
  const taken = new Set<string>()
  const name = (ref: string, text: string) => {
    const label = [...text.trim()].slice(0, 16).join('')
    if (!label || taken.has(label)) return
    taken.add(label)
    names[ref] = label
  }
  for (const p of draft.places) {
    const n = (counts.get(p.kind) ?? 0) + 1
    counts.set(p.kind, n)
    name(`${p.kind}:${n}`, p.name)
  }
  draft.cast.forEach((c, i) => name(`home:${i + 1}`, `${c.name}家`))
  return {
    schema_version: 2,
    appearance_set: 'pr10-village-v1',
    view_mode: '2d',
    environment: { preset: draft.style.preset, vegetation: 'medium' },
    name: [...draft.title].slice(0, 16).join(''),
    seed,
    places: [
      { kind: 'home', count: draft.cast.length, size: 'small' },
      ...[...counts].map(([kind, count]) => ({ kind, count, size: 'medium' })),
    ],
    names,
    constraints: [],
    resources: 'normal-life',
    terrain_source: 'auto',
  }
}

function personaOf(c: StoryCharacter, premise: string): PrefabPersona {
  return {
    name: c.name,
    age: c.age,
    gender: c.gender,
    intro: c.intro,
    tags: c.tags,
    personality: c.personality,
    speech: c.speech,
    background: c.background,
    situation: premise,
    drives: c.drives,
    interests: '',
    occupation: c.occupation,
  }
}

/** 草稿变成一个预制小镇：进镇时开局设置按它走（人数固定是角色数） */
export function scenarioPrefab(draft: StoryDraft | null | undefined): Prefab | null {
  if (!draft?.cast?.length || !draft.beats?.length) return null
  const people = Math.min(draft.cast.length, ROSTER.length)
  return {
    id: 'story',
    name: draft.title,
    description: draft.premise,
    worldview:
      (draft.worldview ? `${draft.worldview}\n` : '') +
      `${draft.premise}\n` +
      '每个人在故事里是谁、想要什么、此刻发生了什么，都写在观察里的 storyline 一栏；' +
      '轮到你说话、要宣布一件事时用 mode_action 交。去哪里、找谁、说什么都由你自己定。这是你们正在经历的事。',
    spec: specOf(draft, 1),
    gameMode: { kind: 'storyline', script: scriptOf(draft) },
    roster: ROSTER.slice(0, people),
    cast: draft.cast.slice(0, people).map((c) => personaOf(c, draft.premise)),
    players: { default: people, min: people, max: people },
    needs: { fullness: true, energy: true, water: true, health: true, weather: true },
    needsDefault: draft.style.needs,
    needsHint: { on: '会饿会渴会困', off: '只看这个故事' },
    options: [],
    limits: { calls_per_day: 600, tokens_per_day: 4_000_000 },
  }
}
