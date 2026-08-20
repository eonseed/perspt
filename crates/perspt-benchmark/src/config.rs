use std::path::Path;

use perspt_sdk::ModelId;

#[derive(Debug, Clone, serde::Serialize)]
pub(super) struct ModelTopology {
    pub(super) actuator: String,
    pub(super) actuator_family: String,
    pub(super) architect: Option<String>,
    pub(super) speculator: Option<String>,
    pub(super) verifier: Option<String>,
    pub(super) adjudicator: Option<String>,
}

pub(super) fn load_config(path: Option<&Path>) -> anyhow::Result<perspt_core::Config> {
    let path = path
        .map(Path::to_path_buf)
        .or_else(perspt_core::paths::resolve_config_file)
        .or_else(perspt_core::paths::config_file);
    match path {
        Some(path) => perspt_core::Config::load_from_path(&path),
        None => anyhow::bail!("benchmark run requires a configured model portfolio"),
    }
}

pub(super) fn configured_topology(
    config: &perspt_core::Config,
    portfolio: &perspt_core::ModelPortfolio,
) -> anyhow::Result<ModelTopology> {
    let models = config.models.as_ref();
    let actuator = models
        .and_then(|models| models.actuator.clone())
        .or_else(|| config.actuator_model.clone())
        .or_else(|| config.model.clone())
        .ok_or_else(|| anyhow::anyhow!("no actuator model is configured"))?;
    let actuator = configured_model_id(&actuator, config, portfolio)?.to_string();
    let optional = |table: Option<String>, flat: &Option<String>| table.or_else(|| flat.clone());
    let qualify_optional = |route: Option<String>| {
        route
            .map(|route| configured_model_id(&route, config, portfolio).map(|id| id.to_string()))
            .transpose()
    };
    let model_name = actuator
        .split_once("::")
        .map(|(_, model)| model)
        .unwrap_or(&actuator);
    let family = perspt_sdk::ModelFamily::from_model_name(model_name);
    let actuator_family = match serde_json::to_value(family)? {
        serde_json::Value::String(label) => label,
        serde_json::Value::Object(fields) => fields
            .into_iter()
            .next()
            .map(|(kind, value)| format!("{kind}:{}", value.as_str().unwrap_or("unknown")))
            .unwrap_or_else(|| "other:unknown".into()),
        _ => "other:unknown".into(),
    };
    Ok(ModelTopology {
        actuator,
        actuator_family,
        architect: qualify_optional(optional(
            models.and_then(|models| models.architect.clone()),
            &config.architect_model,
        ))?,
        speculator: qualify_optional(optional(
            models.and_then(|models| models.speculator.clone()),
            &config.speculator_model,
        ))?,
        verifier: qualify_optional(optional(
            models.and_then(|models| models.verifier.clone()),
            &config.verifier_model,
        ))?,
        adjudicator: qualify_optional(models.and_then(|models| models.adjudicator.clone()))?,
    })
}

pub(super) fn configured_model_id(
    route: &str,
    config: &perspt_core::Config,
    portfolio: &perspt_core::ModelPortfolio,
) -> anyhow::Result<ModelId> {
    if let Some((provider, model)) = route.split_once("::") {
        return Ok(ModelId::new(provider, model));
    }
    let provider_ids = portfolio.provider_ids();
    let provider = config
        .provider
        .as_deref()
        .filter(|provider| provider_ids.iter().any(|candidate| candidate == *provider))
        .map(str::to_owned)
        .or_else(|| (provider_ids.len() == 1).then(|| provider_ids[0].clone()))
        .ok_or_else(|| anyhow::anyhow!("bare model is ambiguous; configure provider::model"))?;
    Ok(ModelId::new(provider, route))
}
