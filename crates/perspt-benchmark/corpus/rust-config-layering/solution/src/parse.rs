use crate::Config;
use std::collections::BTreeSet;

pub fn parse_layer(input: &str) -> Result<Config, String> {
    let mut config = Config::default();
    for (index, raw) in input.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| format!("line {} has no =", index + 1))?;
        match key.trim() {
            "host" => config.host = Some(value.trim().to_owned()),
            "port" => {
                config.port = Some(
                    value
                        .trim()
                        .parse()
                        .map_err(|_| format!("invalid port on line {}", index + 1))?,
                )
            }
            "features" => {
                config.features = Some(
                    value
                        .split(',')
                        .map(str::trim)
                        .filter(|v| !v.is_empty())
                        .map(str::to_owned)
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .collect(),
                )
            }
            other => return Err(format!("unknown key {other}")),
        }
    }
    Ok(config)
}
