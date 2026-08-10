use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// Cell instance lifecycle. Only forward transitions along the happy path are
/// allowed; `Failed` / `RolledBack` are terminal side exits from most states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CellState {
    Discovered,
    Resolved,
    Verified,
    Admitted,
    Staged,
    Starting,
    Ready,
    Active,
    Draining,
    Stopped,
    Failed,
    RolledBack,
}

impl CellState {
    pub fn can_route(self) -> bool {
        matches!(self, Self::Ready | Self::Active)
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Stopped | Self::Failed | Self::RolledBack)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransitionEvent {
    pub from: CellState,
    pub to: CellState,
    pub cell: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Lifecycle {
    cell: String,
    state: CellState,
    events: Vec<TransitionEvent>,
}

impl Lifecycle {
    pub fn new(cell: impl Into<String>) -> Self {
        Self {
            cell: cell.into(),
            state: CellState::Discovered,
            events: Vec::new(),
        }
    }

    pub fn state(&self) -> CellState {
        self.state
    }

    pub fn events(&self) -> &[TransitionEvent] {
        &self.events
    }

    pub fn transition(&mut self, to: CellState, reason: Option<String>) -> Result<&TransitionEvent> {
        if self.state == to {
            // Idempotent no-op: record only once if already there.
            return self
                .events
                .last()
                .ok_or_else(|| Error::Lifecycle("empty event log on noop".into()));
        }
        if !allowed(self.state, to) {
            return Err(Error::Lifecycle(format!(
                "{}: {:?} -> {:?} not allowed",
                self.cell, self.state, to
            )));
        }
        let ev = TransitionEvent {
            from: self.state,
            to,
            cell: self.cell.clone(),
            reason,
        };
        self.state = to;
        self.events.push(ev);
        Ok(self.events.last().expect("just pushed"))
    }

    /// Advance along the standard activation path to `Active`.
    pub fn activate(&mut self) -> Result<()> {
        const PATH: [CellState; 8] = [
            CellState::Discovered,
            CellState::Resolved,
            CellState::Verified,
            CellState::Admitted,
            CellState::Staged,
            CellState::Starting,
            CellState::Ready,
            CellState::Active,
        ];
        if matches!(self.state, CellState::Active) {
            return Ok(());
        }
        let idx = PATH
            .iter()
            .position(|s| *s == self.state)
            .ok_or_else(|| {
                Error::Lifecycle(format!(
                    "{}: cannot activate from {:?}",
                    self.cell, self.state
                ))
            })?;
        for &next in &PATH[idx + 1..] {
            self.transition(next, None)?;
        }
        Ok(())
    }

    pub fn drain_and_stop(&mut self) -> Result<()> {
        if matches!(self.state, CellState::Stopped) {
            return Ok(());
        }
        if self.state.can_route() || matches!(self.state, CellState::Starting | CellState::Staged) {
            if self.state != CellState::Draining {
                self.transition(CellState::Draining, Some("stop".into()))?;
            }
        }
        if self.state == CellState::Draining {
            self.transition(CellState::Stopped, Some("drained".into()))?;
        } else if !self.state.is_terminal() {
            self.transition(CellState::Stopped, Some("stop".into()))?;
        }
        Ok(())
    }

    pub fn fail(&mut self, reason: impl Into<String>) -> Result<()> {
        if matches!(self.state, CellState::Failed) {
            return Ok(());
        }
        self.transition(CellState::Failed, Some(reason.into()))?;
        Ok(())
    }
}

fn allowed(from: CellState, to: CellState) -> bool {
    if matches!(to, CellState::Failed) {
        return !from.is_terminal() || matches!(from, CellState::Draining);
    }
    if matches!(to, CellState::RolledBack) {
        return matches!(
            from,
            CellState::Admitted
                | CellState::Staged
                | CellState::Starting
                | CellState::Ready
                | CellState::Active
                | CellState::Draining
                | CellState::Failed
        );
    }
    match (from, to) {
        (CellState::Discovered, CellState::Resolved) => true,
        (CellState::Resolved, CellState::Verified) => true,
        (CellState::Verified, CellState::Admitted) => true,
        (CellState::Admitted, CellState::Staged) => true,
        (CellState::Staged, CellState::Starting) => true,
        (CellState::Starting, CellState::Ready) => true,
        (CellState::Ready, CellState::Active) => true,
        (CellState::Active, CellState::Draining) => true,
        (CellState::Ready, CellState::Draining) => true,
        (CellState::Starting, CellState::Draining) => true,
        (CellState::Draining, CellState::Stopped) => true,
        (CellState::Admitted, CellState::Stopped)
        | (CellState::Staged, CellState::Stopped)
        | (CellState::Verified, CellState::Stopped)
        | (CellState::Resolved, CellState::Stopped)
        | (CellState::Discovered, CellState::Stopped) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activates_to_active() {
        let mut lc = Lifecycle::new("echo");
        lc.activate().unwrap();
        assert_eq!(lc.state(), CellState::Active);
        assert!(lc.state().can_route());
    }

    #[test]
    fn drain_stop() {
        let mut lc = Lifecycle::new("echo");
        lc.activate().unwrap();
        lc.drain_and_stop().unwrap();
        assert_eq!(lc.state(), CellState::Stopped);
    }

    #[test]
    fn rejects_illegal_jump() {
        let mut lc = Lifecycle::new("echo");
        assert!(lc.transition(CellState::Active, None).is_err());
    }
}
