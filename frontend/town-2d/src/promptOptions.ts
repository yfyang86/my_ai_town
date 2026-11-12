// 设定里「提示词 · 试一试」排故事时要带的地点种类和小镇风格：和故事编辑器用的是同一张表（建镇生成器那边的）。
// 表不小，用到时才取；选择小镇页和游戏页的入口各登记一次。
import { setPromptStoryOptions } from 'town-shared/prompts/studio'

setPromptStoryOptions(async () => {
  const [{ plannerKinds }, library, { STYLE_PRESETS }] = await Promise.all([
    import('../../../scripts/mapgen/plan.mjs'),
    import('../../../data/mapgen/templates.json'),
    import('./map/themes/presets'),
  ])
  return {
    kinds: plannerKinds(library.default.templates)
      .filter((k: { kind: string }) => k.kind !== 'home')
      .map(({ kind, name }: { kind: string; name: string }) => ({ kind, name })),
    presets: STYLE_PRESETS.map(({ id, name }) => ({ id, name })),
  }
})
