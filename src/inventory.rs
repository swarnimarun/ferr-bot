use serde::Deserialize;
use std::collections::HashSet;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Inventory {
    pub services: Vec<Service>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Service {
    pub id: String,
    pub lab: String,
    pub connector: Connector,
    pub status: Status,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Connector {
    Mock,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Healthy,
    Degraded,
    Offline,
}
impl Inventory {
    pub fn parse(json: &str) -> Result<Self, &'static str> {
        let value: Self = serde_json::from_str(json).map_err(|_| "invalid inventory")?;
        let mut ids = HashSet::new();
        if value.services.is_empty() || value.services.len() > 25 {
            return Err("inventory must contain 1..25 services");
        }
        for service in &value.services {
            if !safe_id(&service.id) || !safe_id(&service.lab) || !ids.insert(&service.id) {
                return Err("invalid or duplicate service id");
            }
        }
        Ok(value)
    }
    pub fn find(&self, id: &str) -> Option<&Service> {
        self.services.iter().find(|s| s.id == id)
    }
    pub fn digest(&self) -> String {
        let mut result = String::from("SIMULATED inventory — no hosts contacted\n");
        for s in &self.services {
            result.push_str(&format!("{}/{}: {:?}\n", s.lab, s.id, s.status));
        }
        result
    }
}
fn safe_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 40
        && s.bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_".contains(&c))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn denies_unknown_connectors_and_fields() {
        assert!(Inventory::parse(
            r#"{"services":[{"id":"x","lab":"a","connector":"ssh","status":"healthy"}]}"#
        )
        .is_err());
        assert!(Inventory::parse(r#"{"services":[{"id":"x","lab":"a","connector":"mock","status":"healthy","command":"rm"}]}"#).is_err());
    }
    #[test]
    fn denies_duplicates_and_injection() {
        assert!(Inventory::parse(
            r#"{"services":[{"id":"x;sh","lab":"a","connector":"mock","status":"healthy"}]}"#
        )
        .is_err());
        assert!(Inventory::parse(r#"{"services":[{"id":"x","lab":"a","connector":"mock","status":"healthy"},{"id":"x","lab":"b","connector":"mock","status":"healthy"}]}"#).is_err());
    }
}
