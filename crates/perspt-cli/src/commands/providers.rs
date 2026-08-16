//! Providers command — print the portfolio's capability matrix (PSP-9
//! system 18).
//!
//! Every capability difference between routes is visible here, and every
//! degradation the runtime takes is recorded rather than silently emulated
//! (Gate U). A record that has not been live-probed says so.

use anyhow::Result;
use perspt_core::{Config, ModelPortfolio, ProviderCaps};

/// Print the provider capability matrix for the effective configuration.
/// With `--probe`, additionally run a live behavioral probe per configured
/// model route and print what each route was *observed* to do.
pub async fn run(config_path: Option<std::path::PathBuf>, probe: bool) -> Result<()> {
    let path = config_path.or_else(perspt_core::paths::resolve_config_file);
    let config = match path {
        Some(p) => Config::load_from_path(&p)?,
        None => Config::default(),
    };

    let portfolio = ModelPortfolio::from_config(&config)?;
    if portfolio.is_empty() {
        println!("No providers configured. Add a [providers.<id>] table to config.toml.");
        return Ok(());
    }

    println!("Provider capability matrix ({} route(s)):", portfolio.len());
    println!();
    println!(
        "  {:<12} {:<10} {:>6} {:>7} {:>9} {:>7} {:>7} {:>10} {:>8}",
        "provider",
        "adapter",
        "tools",
        "strict",
        "parallel",
        "stream",
        "cache",
        "context",
        "source",
    );
    for handle in portfolio.handles() {
        let caps = &handle.caps;
        println!(
            "  {:<12} {:<10} {:>6} {:>7} {:>9} {:>7} {:>7} {:>10} {:>8}",
            handle.id,
            handle.adapter,
            mark(caps.tool_calling),
            mark(caps.strict_schema),
            mark(caps.parallel_tool_calls),
            mark(caps.streaming_tool_calls),
            mark(caps.prompt_caching),
            caps.max_context_tokens,
            source(caps),
        );
    }
    println!();
    println!("A route without native tool calling is ineligible as an actuator (Gate U).");

    if probe {
        probe_routes(&config).await?;
    }
    Ok(())
}

/// Live behavioral probes: one scripted two-tool round trip per configured
/// model route, with evidence labelled `behavioral`.
async fn probe_routes(config: &Config) -> Result<()> {
    let Some(models) = config.models.clone() else {
        println!();
        println!("No [models] routes configured; nothing to probe.");
        return Ok(());
    };
    let portfolio = std::sync::Arc::new(ModelPortfolio::from_config(config)?);
    let transport = perspt_agent::GenAiTransport::new(portfolio);
    let mut routes: Vec<(&str, String)> = Vec::new();
    for (role, model) in [
        ("architect", models.architect),
        ("verifier", models.verifier),
        ("speculator", models.speculator),
        ("actuator", models.actuator),
        ("adjudicator", models.adjudicator),
    ] {
        if let Some(model) = model {
            if !routes.iter().any(|(_, existing)| existing == &model) {
                routes.push((role, model));
            }
        }
    }
    anyhow::ensure!(!routes.is_empty(), "no [models] routes configured to probe");

    println!();
    println!("Behavioral probes (live, two-tool round trip per route):");
    println!();
    println!(
        "  {:<12} {:<28} {:>6} {:>6} {:>9} {:>7} {:>6} {:>8}",
        "role", "model", "tools", "both", "parallel", "schema", "turns", "seconds",
    );
    for (role, model) in routes {
        let model: perspt_sdk::ModelId = model
            .parse()
            .map_err(|e| anyhow::anyhow!("route {role}: {e}"))?;
        let report = perspt_agent::probe_route(&transport, &model).await;
        println!(
            "  {:<12} {:<28} {:>6} {:>6} {:>9} {:>7} {:>6} {:>7.2}s",
            role,
            report.model,
            mark(report.tool_call_round_trip),
            mark(report.multi_tool_selection),
            mark(report.parallel_tool_calls),
            mark(report.schema_arguments_valid),
            report.turns,
            report.total_seconds,
        );
        if let Some(error) = &report.error {
            println!("    error: {error}");
        }
    }
    println!();
    println!("Evidence source: behavioral (observed on live round trips, not declared).");
    Ok(())
}

fn mark(supported: bool) -> &'static str {
    if supported {
        "yes"
    } else {
        "no"
    }
}

fn source(caps: &ProviderCaps) -> &'static str {
    if caps.probed {
        "probed"
    } else {
        "declared"
    }
}
