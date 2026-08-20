#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Config {
    pub host: Option<String>,
    pub port: Option<u16>,
    pub features: Option<Vec<String>>,
}

impl Config {
    pub fn apply(self, later: Self) -> Self {
        Self {
            host: later.host.or(self.host),
            port: later.port.or(self.port),
            features: later.features.or(self.features),
        }
    }
}
