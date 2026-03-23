use std::fmt;

#[derive(Serialize, Deserialize, Clone)]
pub struct Config {
    pub host: String,
    pub port: u16,
}

impl Config {
    pub fn new() -> Self {
        Config { host: "localhost".into(), port: 8080 }
    }
}

impl fmt::Display for Config {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}:{}", self.host, self.port)
    }
}

pub fn validate_port(port: u16) -> Result<(), String> {
    if port == 0 {
        return Err("port cannot be zero".into());
    }
    Ok(())
}
