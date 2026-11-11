// 故事模式的音乐：故事是什么调子剧本说了不算数，平时交给天气和时段；收场放终章。
// 节拍到点响一下公告的短提示，里程碑达成响一下亮的。
import type { GameModeMusic } from '.'

export const storylineMusic: GameModeMusic<'storyline'> = {
  order: ['story/finale'],
  quietDeaths: false,
  cell(view) {
    return view.ended ? 'story/finale' : null
  },
  sting(view, before) {
    const done = (v: typeof view) => (v.milestones ?? []).filter((m) => m.done).length
    if (done(view) > done(before)) return 'tide-rise'
    const fired = (v: typeof view) => v.beats.filter((b) => b.state !== 'upcoming').length
    return fired(view) > fired(before) ? 'tide-announce' : null
  },
}
