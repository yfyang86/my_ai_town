//! 提示词调校在管理服务这边的几步：读、存、看拼好的提示词、拿一句话试生成一个人物。
//! 设置是应用的配置，所有小镇共用，存在模型服务配置旁边的 prompts.json；怎么拼在 agent::tuning。
//! 电脑版经 town-map 调，手机上的管理服务直接调；写故事的试一试走 host::story（请求里带 prompt）。
use crate::application::Application;
use crate::HostError;
use agent::tuning::{self, PromptSettings, Target, Tuning};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn path(app: &Application) -> PathBuf {
    Path::new(&app.providers_path).with_file_name("prompts.json")
}

/// 存着的设置；没存过、读不了都当默认（提示词坏了不该让生成用不了）。
pub fn load(app: &Application) -> PromptSettings {
    std::fs::read(path(app))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<PromptSettings>(&bytes).ok())
        .unwrap_or_default()
        .tidy()
}

fn target_of(body: &Value) -> Result<Target, HostError> {
    body["target"]
        .as_str()
        .and_then(Target::of)
        .ok_or_else(|| HostError::Runtime("target 要是 story 或 character".into()))
}

/// 请求里带的一份调法（试一试、看拼好的提示词时用的，不存）
pub fn tuning_of(value: &Value) -> Result<Tuning, HostError> {
    serde_json::from_value::<Tuning>(value.clone())
        .map(Tuning::tidy)
        .map_err(|e| HostError::Runtime(format!("提示词设置的格式不对：{e}")))
}

fn view(settings: &PromptSettings) -> Value {
    let about = |target: Target| {
        json!({
            "template": tuning::default_template(target),
            "placeholders": tuning::placeholders(target)
                .iter()
                .map(|(mark, means)| json!({ "mark": mark, "means": means }))
                .collect::<Vec<_>>(),
            "warnings": settings.of(target).warnings(target),
        })
    };
    json!({
        "settings": settings,
        "defaults": { "story": about(Target::Story), "character": about(Target::Character) },
        "limits": { "module": tuning::MODULE_MAX_CHARS, "pro": tuning::PRO_MAX_CHARS },
    })
}

/// 存着的设置，连同默认的模板、占位符说明和字数上限
pub fn read(app: &Application) -> Result<Value, HostError> {
    Ok(view(&load(app)))
}

/// 存一份设置（整份换掉）；回的和 read 一样
pub fn write(app: &Application, body: &Value) -> Result<Value, HostError> {
    let settings = serde_json::from_value::<PromptSettings>(body["settings"].clone())
        .map_err(|e| HostError::Runtime(format!("提示词设置的格式不对：{e}")))?
        .tidy();
    let text =
        serde_json::to_vec_pretty(&settings).map_err(|e| HostError::Runtime(e.to_string()))?;
    crate::persistence::atomic_write(&path(app), &text)?;
    Ok(view(&settings))
}

/// 这份调法拼出来的整份模板（占位符没换，输入也没接上），连同提醒
pub fn preview(body: &Value) -> Result<Value, HostError> {
    let target = target_of(body)?;
    let tuning = tuning_of(&body["tuning"])?;
    Ok(json!({ "template": tuning.template(target), "warnings": tuning.warnings(target) }))
}

/// 拿一句话照这份调法生成一个人物（不存）：试一试、比一比用。mock 时不调模型。
pub fn try_character(app: &Application, body: &Value, mock: bool) -> Result<Value, HostError> {
    let brief = body["brief"]
        .as_str()
        .map(str::trim)
        .filter(|b| !b.is_empty() && b.chars().count() <= 300)
        .ok_or_else(|| HostError::Runtime("一句话描述须为 1–300 字".into()))?;
    let tuning = match &body["tuning"] {
        Value::Null => load(app).character,
        given => tuning_of(given)?,
    };
    if mock {
        let fields = crate::library::mock_generated(brief, &[])?;
        return Ok(json!({ "fields": fields, "model": "mock", "provider": "mock" }));
    }
    let providers = crate::config::load_providers(&app.providers_path)?;
    let credentials = crate::config::load_credentials(&app.credentials_path)?;
    let target = crate::llm::LlmTarget::active(&providers, &credentials)
        .ok_or_else(|| HostError::Config("请先在首页的「设定」里连接一个模型".into()))?;
    let prompt = agent::completion::template_prompt_with(brief, &[], &[], &tuning);
    let fields = crate::llm::chat_completion(&target, &prompt)
        .and_then(|text| agent::completion::parse_template(&text))
        .map_err(HostError::from)
        .and_then(crate::library::tidy_generated)?;
    Ok(json!({ "fields": fields, "model": target.model, "provider": target.provider }))
}
