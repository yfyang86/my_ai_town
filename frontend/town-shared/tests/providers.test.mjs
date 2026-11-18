import test from 'node:test'
import assert from 'node:assert/strict'
import { PRESETS, providerRow, quickModelNames, rememberProviderModel } from '../src/providers.ts'

test('provider presets use current model IDs and no longer offer retired defaults', () => {
  const byId = new Map(PRESETS.map((preset) => [preset.id, preset]))
  assert.deepEqual(byId.get('deepseek').models, ['deepseek-flash', 'deepseek-v4-pro'])
  assert.equal(byId.get('deepseek').defaultModel, 'deepseek-flash')
  assert.deepEqual(byId.get('kimi').models, ['kimi-k3.1-pro', 'kimi-k3.1-pro-highspeed', 'kimi-k3'])
  assert.equal(byId.get('zhipu-glm').defaultModel, 'glm-5.3')
  assert.ok(byId.get('zhipu-glm').models.includes('glm-5.3-flash'))
  assert.deepEqual(byId.get('minimax').models, ['MiniMax-M3.1-Flash', 'MiniMax-M3'])
  assert.equal(byId.get('minimax').baseUrl, 'https://api.minimax.cn/v1')
  assert.ok(byId.get('alibaba-bailian').models.includes('qwen3.8-max'))
  assert.ok(byId.get('alibaba-bailian').models.includes('qwen3.8-flash'))

  const offered = PRESETS.flatMap((preset) => [...preset.models, preset.defaultModel])
  for (const retired of [
    'deepseek-chat',
    'deepseek-reasoner',
    'deepseek-v4-flash',
    'kimi-k2.5',
    'MiniMax-M2.7',
  ])
    assert.ok(!offered.includes(retired), `${retired} should not be offered`)
})

test('a custom model is remembered, kept on its provider, and offered for later selection', () => {
  const original = {
    id: 'deepseek',
    name: 'DeepSeek',
    base_url: 'https://api.deepseek.com',
    key_mask: 'sk-a…7890',
    model: 'deepseek-flash',
    models: ['existing-custom'],
  }
  const added = rememberProviderModel(providerRow(original), 'vendor/new-model')
  assert.equal(added.model, 'vendor/new-model')
  assert.deepEqual(added.models, ['existing-custom', 'vendor/new-model'])
  assert.deepEqual(providerRow({ ...original, model: added.model, models: added.models }), added)
  assert.deepEqual(quickModelNames('deepseek', undefined, added), [
    'vendor/new-model',
    'existing-custom',
    'deepseek-flash',
    'deepseek-v4-pro',
  ])
})
