//! In-process event bus — lifecycle and capability announcements (no network).

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

use crate::lifecycle::CellState;

const DEFAULT_CAP: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BusEvent {
    CellState {
        cell: String,
        state: CellState,
    },
    CapabilityAvailable {
        cell: String,
        capability: String,
        version: String,
    },
    BindingChanged {
        generation: u64,
        count: usize,
    },
}

#[derive(Debug, Clone)]
pub struct EventBus {
    events: VecDeque<(u64, BusEvent)>,
    cap: usize,
    next_seq: u64,
}

impl Default for EventBus {
    fn default() -> Self {
        Self::with_capacity(DEFAULT_CAP)
    }
}

impl EventBus {
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            events: VecDeque::new(),
            cap: cap.max(1),
            next_seq: 1,
        }
    }

    pub fn publish(&mut self, event: BusEvent) -> u64 {
        let seq = self.next_seq;
        self.next_seq = self.next_seq.saturating_add(1);
        if self.events.len() >= self.cap {
            self.events.pop_front();
        }
        self.events.push_back((seq, event));
        seq
    }

    pub fn snapshot(&self) -> Vec<(u64, BusEvent)> {
        self.events.iter().cloned().collect()
    }

    pub fn since(&self, after_seq: u64) -> Vec<(u64, BusEvent)> {
        self.events
            .iter()
            .filter(|(s, _)| *s > after_seq)
            .cloned()
            .collect()
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn since_filters() {
        let mut bus = EventBus::with_capacity(8);
        let a = bus.publish(BusEvent::BindingChanged {
            generation: 1,
            count: 1,
        });
        let _b = bus.publish(BusEvent::BindingChanged {
            generation: 2,
            count: 2,
        });
        assert_eq!(bus.since(a).len(), 1);
    }
}
