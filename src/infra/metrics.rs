use prometheus::{Encoder, Registry, TextEncoder};

pub struct AppMetrics {
    registry: Registry,
}

impl AppMetrics {
    pub fn new() -> Result<Self, prometheus::Error> {
        let registry = Registry::new();
        Ok(Self { registry })
    }

    pub fn render(&self) -> String {
        let encoder = TextEncoder::new();
        let mut buf = Vec::new();
        encoder.encode(&self.registry.gather(), &mut buf).unwrap();
        String::from_utf8(buf).unwrap()
    }
}
