import type { SettingsPage } from '../settingsCenter'
import { button, el } from '../dom'
import { openPromptStudio } from '../prompts/studio'
import { settingRow } from './controls'

// 设定中心的「提示词」页：只是一个入口。提示词是应用的设置、所有小镇共用，自己读写、自己保存
// （prompts/studio.ts），不走设定中心底部的保存栏。
export function createPromptsPage(): SettingsPage {
  const page = el('section', '', 'spage')
  const rows = el('div', '', 'set-rows')
  rows.append(
    settingRow(
      '写故事 · 生成人物',
      '换一种风格、逐段改，或者整份自己写；拿同一段故事让两份提示词各跑一次，并排比。',
      button('打开', () => openPromptStudio())
    )
  )
  page.append(
    el('h2', '提示词'),
    el('p', '模型写故事、写人物时照着的那段话。所有小镇共用，改了以后下一次生成就用上。', 'hint'),
    rows
  )
  return {
    id: 'prompts',
    group: 'model',
    navLabel: '提示词',
    title: '提示词',
    el: page,
    isDirty: () => false,
    save() {},
    discard() {},
  }
}
