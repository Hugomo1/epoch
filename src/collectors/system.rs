use crate::types::SystemMetrics;
use color_eyre::Result;

pub struct SystemCollector {
    sys: sysinfo::System,
    tx: tokio::sync::mpsc::Sender<SystemMetrics>,
    initialized: bool,
}

impl SystemCollector {
    pub fn new(tx: tokio::sync::mpsc::Sender<SystemMetrics>) -> Self {
        Self {
            sys: sysinfo::System::new(),
            tx,
            initialized: false,
        }
    }

    pub async fn collect(&mut self) -> Result<()> {
        // First call only: double-refresh for accurate CPU reading
        if !self.initialized {
            self.sys.refresh_cpu_all();
            self.sys.refresh_memory();
            tokio::time::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL).await;
            self.initialized = true;
        }

        self.sys.refresh_cpu_all();
        self.sys.refresh_memory();

        let metrics = SystemMetrics {
            cpu_usage: self.sys.global_cpu_usage() as f64,
            memory_used: self.sys.used_memory(),
            memory_total: self.sys.total_memory(),
            gpus: vec![],
        };

        // Ignore send error if receiver dropped
        self.tx.send(metrics).await.ok();

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_system_collector_creation() {
        let (tx, _rx) = tokio::sync::mpsc::channel(1);
        let _collector = SystemCollector::new(tx);
    }

    #[tokio::test]
    async fn test_collect_produces_metrics() {
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        let mut collector = SystemCollector::new(tx);

        collector.collect().await.expect("collect should succeed");

        let metrics = rx.recv().await.expect("should receive metrics");
        assert!(metrics.memory_total > 0, "memory_total should be > 0");
    }

    #[tokio::test]
    async fn test_collect_cpu_usage_range() {
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        let mut collector = SystemCollector::new(tx);

        collector.collect().await.expect("collect should succeed");

        let metrics = rx.recv().await.expect("should receive metrics");
        assert!(
            metrics.cpu_usage >= 0.0 && metrics.cpu_usage <= 100.0,
            "cpu_usage should be in range [0.0, 100.0], got {}",
            metrics.cpu_usage
        );
    }

    #[tokio::test]
    async fn test_collect_sends_empty_gpus() {
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        let mut collector = SystemCollector::new(tx);

        collector.collect().await.expect("collect should succeed");

        let metrics = rx.recv().await.expect("should receive metrics");
        assert!(
            metrics.gpus.is_empty(),
            "gpus vec should be empty in SystemCollector"
        );
    }
}
