use prometheus::{Encoder, IntCounterVec, Opts, Registry, TextEncoder};

pub struct AppMetrics {
    registry: Registry,
    page_views_total: IntCounterVec,
}

impl AppMetrics {
    pub fn new() -> Result<Self, prometheus::Error> {
        let registry = Registry::new();
        let page_views_total = IntCounterVec::new(
            Opts::new("page_views_total", "Total number of page views"),
            &["page"],
        )?;
        registry.register(Box::new(page_views_total.clone()))?;
        Ok(Self {
            registry,
            page_views_total,
        })
    }

    pub fn inc_page_view(&self, page: &str) {
        self.page_views_total.with_label_values(&[page]).inc();
    }

    pub fn render(&self) -> String {
        let encoder = TextEncoder::new();
        let mut buf = Vec::new();
        encoder.encode(&self.registry.gather(), &mut buf).unwrap();
        String::from_utf8(buf).unwrap()
    }

    #[cfg(test)]
    fn page_view_count(&self, page: &str) -> f64 {
        self.page_views_total.with_label_values(&[page]).get() as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inc_page_view_increments_counter() {
        let metrics = AppMetrics::new().expect("metrics");
        let initial = metrics.page_view_count("home");
        metrics.inc_page_view("home");
        assert_eq!(metrics.page_view_count("home"), initial + 1.0);
    }
}
