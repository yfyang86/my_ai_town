import type * as THREE from 'three'
import type { GameModeView, StorylineSeat, StorylineView } from 'town-shared/protocol'
import { el } from 'town-shared/dom'
import { residentPortrait } from 'town-shared/portraits'
import type { GameModeLook } from '.'
import './storyline.css'

// 故事模式的画面：不改着色，地图上一层界面（上帝视角）。
// 顶上一条横幅：故事的名字、此刻到了哪一幕。左边一列角色：头像、名字、在故事里是谁，轮到谁亮绿圈。
// 右边一块板子：时间线（过去的、正在演的、还没到的节拍）和里程碑（达成的打勾，写着谁宣布的、怎么说的；
// 收场时没达成的划掉），收场后底下是收场白。底下一张卡片放戏里刚说的那段话和「心里」。
// 名册每来一拍整层重画，状态没变时场景不会叫它。

/** 世界分钟写成「第 2 天 17:00」；只有一天的故事不写第几天 */
function stamp(at: number, first: number, days: boolean) {
  const day = Math.floor(at / 1440) - Math.floor(first / 1440) + 1
  const minute = Math.round(at) % 1440
  const hm = `${String(Math.floor(minute / 60)).padStart(2, '0')}:${String(minute % 60).padStart(2, '0')}`
  return days ? `第 ${day} 天 ${hm}` : hm
}

export class StorylineLook implements GameModeLook {
  readonly waterAnywhere = false
  private layer: HTMLElement | null = null
  private view: StorylineView | null = null

  shadeGround(_material: THREE.Material, _surface: 'land' | 'sea') {}
  shadeStanding(_material: THREE.Material, _who: 'thing' | 'villager') {}

  mount(layer: HTMLElement) {
    this.layer = el('div', '', 'st-hud')
    this.layer.hidden = true
    layer.append(this.layer)
    this.render()
  }

  apply(view: GameModeView | null) {
    this.view = view?.kind === 'storyline' ? view : null
    this.render()
  }

  private seat(n: number | undefined): StorylineSeat | undefined {
    return n ? this.view?.seats[n - 1] : undefined
  }

  private render() {
    const root = this.layer
    if (!root) return
    const v = this.view
    root.hidden = !v
    root.replaceChildren()
    if (!v) return
    root.append(this.banner(v), this.cast(v), this.board(v))
    const stage = this.stage(v)
    if (stage) root.append(stage)
  }

  private banner(v: StorylineView) {
    const box = el('div', '', v.ended ? 'st-banner ended' : 'st-banner')
    box.append(el('b', v.title), el('span', v.notice))
    return box
  }

  /** 左边的角色：头像、名字、身份；轮到谁说话亮绿圈 */
  private cast(v: StorylineView) {
    const box = el('div', '', 'st-cast')
    for (const s of v.seats) {
      const row = el('div', '', 'st-seat')
      if (s.gone) row.classList.add('gone')
      if (v.turn === s.seat) row.classList.add('turn')
      const face = residentPortrait(s.id, s.name)
      face.classList.add('st-face')
      row.append(face, el('b', s.name), el('small', s.role))
      if (s.gone) row.append(el('i', '不在了'))
      box.append(row)
    }
    return box
  }

  /** 右边的板子：时间线、里程碑，收场后加收场白 */
  private board(v: StorylineView) {
    const box = el('div', '', 'st-board')
    const first = v.beats[0]?.at ?? 0
    const last = v.beats.at(-1)?.at ?? first
    const days = Math.floor(last / 1440) > Math.floor(first / 1440)
    box.append(el('h4', '时间线'))
    const line = el('ol', '', 'st-beats')
    for (const b of v.beats) {
      const item = el('li', '', b.state)
      item.append(el('time', stamp(b.at, first, days)), el('b', b.title))
      if (b.place) item.append(el('small', b.place))
      line.append(item)
    }
    box.append(line)
    if (v.milestones?.length) {
      box.append(el('h4', '里程碑'))
      const list = el('ul', '', 'st-marks')
      for (const m of v.milestones) {
        const item = el('li', '', m.done ? 'done' : m.missed ? 'missed' : '')
        item.append(el('i', m.done ? '✓' : m.missed ? '✕' : '·'), el('b', m.title))
        const who = this.seat(m.by)?.name
        item.append(el('small', m.done && m.text ? `${who ?? ''}：${m.text}` : m.detail))
        list.append(item)
      }
      box.append(list)
    }
    if (v.ended && v.epilogue) box.append(el('p', v.epilogue, 'st-epilogue'))
    return box
  }

  /** 底下的卡片：戏里最后说的那段话和心里；轮到的人还在想时写「X 正在想…」 */
  private stage(v: StorylineView) {
    const current = v.beats.find((b) => b.state === 'current')
    const line = v.lines?.at(-1)
    // 戏散了以后那段话不一直挂着
    if (!current && !v.ended) return null
    if (!line && !v.turn) return null
    if (v.ended && !line) return null
    const card = el('div', '', 'st-stage')
    if (line && (v.ended || line.occasion === current?.title)) {
      const s = this.seat(line.seat)
      const head = el('div', '', 'st-stage-head')
      if (s) {
        const face = residentPortrait(s.id, s.name)
        face.classList.add('st-face')
        head.append(face, el('b', s.name), el('small', s.role))
      }
      head.append(el('em', line.away ? `${line.occasion} · 没到场` : line.occasion))
      card.append(head, el('p', line.text))
      if (line.reason) {
        const mind = el('div', '', 'st-mind')
        mind.append(el('b', '心里'), el('span', line.reason))
        card.append(mind)
      }
    }
    const next = this.seat(v.turn)
    if (next) card.append(el('div', `●●● ${next.name} 正在想…`, 'st-next'))
    return card.childElementCount ? card : null
  }
}
