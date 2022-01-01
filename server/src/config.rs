use serde::Deserialize;
use std::fs::File;
use std::io::Read;
use std::path::Path;

#[derive(Deserialize)]
pub struct SharkAuth {
    pub user: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct Config {
    pub user_auth: Vec<String>,
    pub shark: SharkAuth,
}

impl Config {
    pub fn from_file<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
        let path = path.as_ref();
        let mut f = File::open(path)?;
        let mut buf: Vec<u8> = Vec::new();
        f.read_to_end(&mut buf)?;
        let config: Self = toml::from_slice(&buf)?;

        Ok(config)
    }
}
