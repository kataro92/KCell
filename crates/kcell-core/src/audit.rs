//! Bounded audit trail — security/ops events without a logging framework dependency.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_CAP: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditKind {
    Registered,
    Admitted,
    AdmitDenied,
    Activated,
    Stopped,
    BindingApplied,
    BindingRejected,
    Invoked,
    InvokeFailed,
    PolicyGrant,
    Discovered,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEvent {
    pub seq: u64,
    pub at_ms: u64,
    pub kind: AuditKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cell: Option<String>,
    pub detail: String,
}

#[derive(Debug, Clone)]
pub struct AuditLog {
    events: VecDeque<AuditEvent>,
    cap: usize,
    next_seq: u64,
}

impl Default for AuditLog {
    fn default() -> Self {
        Self::with_capacity(DEFAULT_CAP)
    }
}

impl AuditLog {
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            events: VecDeque::with_capacity(cap.clamp(1, DEFAULT_CAP)),
            cap: cap.max(1),
            next_seq: 1,
        }
    }

    pub fn record(&mut self, kind: AuditKind, cell: Option<String>, detail: impl Into<String>) {
        let ev = AuditEvent {
            seq: self.next_seq,
            at_ms: now_ms(),
            kind,
            cell,
            detail: detail.into(),
        };
        self.next_seq = self.next_seq.saturating_add(1);
        if self.events.len() >= self.cap {
            self.events.pop_front();
        }
        self.events.push_back(ev);
    }

    pub fn events(&self) -> &VecDeque<AuditEvent> {
        &self.events
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn clear(&mut self) {
        self.events.clear();
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_drops_oldest() {
        let mut log = AuditLog::with_capacity(2);
        log.record(AuditKind::Registered, Some("a".into()), "1");
        log.record(AuditKind::Activated, Some("b".into()), "2");
        log.record(AuditKind::Invoked, Some("c".into()), "3");
        assert_eq!(log.len(), 2);
        assert_eq!(log.events().front().unwrap().cell.as_deref(), Some("b"));
    }
}
