//! 故事编辑器在管理服务这边的一步：提纲交给模型写成故事草稿，或者把页面改过的草稿再理一遍。
//! 电脑版经 town-map 调，手机上的管理服务直接调；理草稿的规则在 agent 的 story.rs。
use crate::application::Application;
use crate::HostError;
use agent::story::{self, Choices, CAST, DAYS, OUTLINE_MAX_CHARS};
use serde_json::{json, Value};

/// 请求里带 draft：理一遍、查一遍交回去，不调模型。
/// 带 outline：调一次模型（mock 时用固定的回复），回复里补上模型和服务商。
pub fn draft(app: &Application, body: &Value, mock: bool) -> Result<Value, HostError> {
    let mut choices = Choices::of(body).map_err(HostError::Runtime)?;
    // 提示词照设定里调的来；请求里带了 prompt（设定里「试一试」）就用带的那一份
    choices.tuning = match &body["prompt"] {
        Value::Null => crate::prompts::load(app).story,
        given => crate::prompts::tuning_of(given)?,
    };
    if body["draft"].is_object() {
        let draft = story::tidy(&body["draft"], &choices).map_err(HostError::Runtime)?;
        return Ok(json!({ "draft": draft }));
    }
    let outline = body["outline"]
        .as_str()
        .map(str::trim)
        .filter(|o| !o.is_empty() && o.chars().count() <= OUTLINE_MAX_CHARS)
        .ok_or_else(|| HostError::Runtime(format!("故事或提纲须为 1–{OUTLINE_MAX_CHARS} 字")))?;
    let within = |value: &Value, range: &std::ops::RangeInclusive<u64>, what: &str| {
        value.as_u64().filter(|n| range.contains(n)).ok_or_else(|| {
            HostError::Runtime(format!("{what}要在 {}–{} 之间", range.start(), range.end()))
        })
    };
    let cast = within(&body["cast"], &CAST, "角色数")?;
    let days = within(&body["days"], &DAYS, "天数")?;
    if mock {
        let reply = story::mock_reply(outline, cast, &choices);
        let draft = story::parse(&reply, days, &choices)?;
        return Ok(json!({ "draft": draft, "model": "mock", "provider": "mock" }));
    }
    let providers = crate::config::load_providers(&app.providers_path)?;
    let active = providers
        .active
        .as_ref()
        .ok_or_else(|| HostError::Config("请先在首页的「设定」里连接一个模型".into()))?;
    let provider = providers
        .providers
        .iter()
        .find(|p| p.id == active.provider)
        .map(|p| p.name.clone())
        .ok_or_else(|| HostError::Config("服务商不存在".into()))?;
    let credentials = crate::config::load_credentials(&app.credentials_path)?;
    let target = crate::llm::LlmTarget::active(&providers, &credentials)
        .ok_or_else(|| HostError::Config("服务商不存在".into()))?;
    let prompt = story::prompt(outline, cast, days, &choices);
    let reply = crate::llm::chat_completion(&target, &prompt)?;
    let draft = story::parse(&reply, days, &choices)?;
    Ok(json!({ "draft": draft, "model": target.model, "provider": provider }))
}
