use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::envelope::AgentId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegisteredAgent {
    pub id: AgentId,
    pub name: String,
    pub connected_at: u64,
}

#[derive(Default, Debug)]
pub struct Registry {
    by_name: HashMap<String, RegisteredAgent>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, name: impl Into<String>) -> RegisteredAgent {
        let name = name.into();
        let agent = RegisteredAgent {
            id: AgentId::new(),
            name: name.clone(),
            connected_at: now_unix_millis(),
        };
        self.by_name.insert(name, agent.clone());
        agent
    }

    pub fn restore(
        &mut self,
        name: impl Into<String>,
        id: AgentId,
        last_seen: u64,
    ) -> RegisteredAgent {
        let name = name.into();
        let agent = RegisteredAgent {
            id,
            name: name.clone(),
            connected_at: last_seen,
        };
        self.by_name.insert(name, agent.clone());
        agent
    }

    pub fn resolve(&self, name: &str) -> Option<&RegisteredAgent> {
        self.by_name.get(name)
    }

    pub fn unregister(&mut self, name: &str) -> Option<RegisteredAgent> {
        self.by_name.remove(name)
    }

    pub fn len(&self) -> usize {
        self.by_name.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_name.is_empty()
    }

    pub fn agents(&self) -> Vec<RegisteredAgent> {
        let mut agents: Vec<_> = self.by_name.values().cloned().collect();
        agents.sort_by(|left, right| left.name.cmp(&right.name));
        agents
    }
}

fn now_unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time is before unix epoch")
        .as_millis()
        .try_into()
        .expect("unix millis fits in u64")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newest_registration_wins_name() {
        let mut registry = Registry::new();

        let first = registry.register("engineer");
        let second = registry.register("engineer");

        assert_ne!(first.id, second.id);
        assert_eq!(registry.resolve("engineer"), Some(&second));
        assert_eq!(registry.len(), 1);
    }
}
