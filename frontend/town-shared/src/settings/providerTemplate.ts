import type { ProviderPreset } from '../providers'
export function providerTemplate(preset: ProviderPreset) {
  return `
    <label class="prov-field">服务地址
      <input id="p-url-${preset.id}" class="p-url" spellcheck="false">
    </label>
    <label class="prov-field">API 密钥
      <input id="p-key-${preset.id}" class="p-key" type="password" spellcheck="false" autocomplete="off">
    </label>
    <label class="prov-field">默认模型
      <input id="p-model-${preset.id}" class="p-model" list="p-models-${preset.id}" spellcheck="false">
      <datalist id="p-models-${preset.id}"></datalist>
    </label>
    <button type="button" class="p-test">测试连接</button>
    <div class="p-added-models" aria-label="玩家添加的模型"></div>
    <div class="prov-field p-add-field">
      <label for="p-new-model-${preset.id}">添加模型</label>
      <div class="p-add-row">
        <input id="p-new-model-${preset.id}" class="p-new-model" spellcheck="false" placeholder="输入服务商的模型 ID">
        <button type="button" class="p-add-model">添加</button>
      </div>
    </div>
    <p class="hint p-model-hint">默认模型会在选择这个服务时自动填上；添加的模型会出现在默认模型和村民模型的候选里。</p>
    <div class="form-status p-test-status"></div>`
}
