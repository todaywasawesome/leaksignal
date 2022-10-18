use std::{collections::HashMap, sync::RwLock};

use proxy_wasm::{hostcalls, types::MetricType};

lazy_static::lazy_static! {
    static ref DEFINED_METRICS: RwLock<HashMap<String, u32>> = RwLock::new(HashMap::new());
}

pub struct Metric(u32);

impl Metric {
    pub fn lookup_or_define(name: impl AsRef<str>, metric_type: MetricType) -> Self {
        let name = name.as_ref();
        let map = DEFINED_METRICS.read().unwrap();
        if let Some(metric_id) = map.get(name) {
            return Metric(*metric_id);
        }
        drop(map);
        let mut map = DEFINED_METRICS.write().unwrap();
        // check again in case of race condition
        if let Some(metric_id) = map.get(name) {
            return Metric(*metric_id);
        }
        // envoy's implementation only panics on an invalid call, which shouldn't be possible
        let metric_id = hostcalls::define_metric(metric_type, name).expect("define_metric failed");
        map.insert(name.to_string(), metric_id);
        Metric(metric_id)
    }

    #[allow(dead_code)]
    pub fn set_value(&self, value: u64) {
        hostcalls::record_metric(self.0, value).expect("failed to record_metric");
    }

    pub fn increment(&self, value: i64) {
        hostcalls::increment_metric(self.0, value).expect("failed to increment_metric");
    }
}
