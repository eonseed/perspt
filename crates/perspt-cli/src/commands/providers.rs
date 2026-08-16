//! Providers command — print the portfolio's capability matrix (PSP-9
//! system 18).
//!
//! Every capability difference between routes is visible here, and every
//! degradation the runtime takes is recorded rather than silently emulated
//! (Gate U). A record that has not been live-probed says so.

use anyhow::Result;
use perspt_core::{Config, ModelPortfolio, ProviderCaps};

/// Print the provider capability matrix for the effective configuration.
pub async fn run(config_path: Option<std::path::PathBuf>) -> Result<()> {
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
    println!("A route without tool support falls back to bundle mode with the reason recorded.");
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
