use std::{fmt::Write, ops::Deref, sync::Arc};

use arc_swap::{ArcSwap, Guard};
use sha2::{Digest, Sha256};

use crate::low_entropy_hash::LowEntropyHash;
pub use leakpolicy::*;

lazy_static::lazy_static! {
    static ref POLICY: ArcSwap<Option<(String, Policy)>> = {
        ArcSwap::new(Arc::new(None))
    };
}

pub struct PolicyRef(Guard<Arc<Option<(String, Policy)>>>);

impl Deref for PolicyRef {
    type Target = Policy;

    fn deref(&self) -> &Self::Target {
        &(&**self.0).as_ref().unwrap().1
    }
}

impl PolicyRef {
    #[allow(dead_code)]
    pub fn policy_id(&self) -> &str {
        &*(&**self.0).as_ref().unwrap().0
    }
}

pub fn policy() -> Option<PolicyRef> {
    let policy = POLICY.load();
    if policy.is_none() {
        return None;
    }
    Some(PolicyRef(policy))
}

pub fn update_policy(policy_id: String, policy: Policy) {
    POLICY.store(Arc::new(Some((policy_id, policy))));
}

pub fn evaluate_report_style(style: DataReportStyle, input: &str) -> Option<String> {
    match style {
        DataReportStyle::Raw => Some(input.to_string()),
        DataReportStyle::PartialSha256 { report_bits } => Some(
            LowEntropyHash::new(report_bits)
                .update_chained(input.as_bytes())
                .finalize()
                .to_string(),
        ),
        DataReportStyle::Sha256 => {
            let mut out = String::with_capacity(64);
            for byte in Sha256::new().chain_update(input.as_bytes()).finalize() {
                write!(&mut out, "{byte:02X}").ok()?;
            }
            Some(out)
        }
        DataReportStyle::None => None,
    }
}
