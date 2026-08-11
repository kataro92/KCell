//! A2A Agent Card builder from Host discover providers.

use serde_json::{json, Value};

pub struct CardConfig<'a> {
    pub name: &'a str,
    pub description: &'a str,
    pub url: &'a str,
    pub version: &'a str,
}

pub fn build_agent_card(cfg: &CardConfig<'_>, providers: &Value) -> Value {
    let skills = skills_from_providers(providers);
    json!({
        "name": cfg.name,
        "description": cfg.description,
        "url": cfg.url,
        "version": cfg.version,
        "protocolVersion": "0.3.0",
        "capabilities": {
            "streaming": false,
            "pushNotifications": false
        },
        "defaultInputModes": ["application/json", "text/plain"],
        "defaultOutputModes": ["application/json", "text/plain"],
        "skills": skills
    })
}

pub fn skills_from_providers(providers: &Value) -> Vec<Value> {
    let Some(arr) = providers.as_array() else {
        return Vec::new();
    };
    let mut skills = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for p in arr {
        let cell = p.get("cell").and_then(|v| v.as_str()).unwrap_or("");
        let capability = p.get("capability").and_then(|v| v.as_str()).unwrap_or("");
        if capability.is_empty() {
            continue;
        }
        if !seen.insert(capability.to_string()) {
            continue;
        }
        let version = p
            .get("capabilityVersion")
            .and_then(|v| v.as_str())
            .unwrap_or("1");
        skills.push(json!({
            "id": capability,
            "name": capability,
            "description": format!(
                "KCell capability `{capability}@{version}` (e.g. from cell `{cell}`)"
            ),
            "tags": ["kcell", cell],
            "examples": []
        }));
    }
    skills
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skills_from_discover_json() {
        let providers = json!([
            {
                "cell": "echo-cell",
                "capability": "echo",
                "capabilityVersion": "1"
            },
            {
                "cell": "other",
                "capability": "echo",
                "capabilityVersion": "1"
            },
            {
                "cell": "brain",
                "capability": "llm",
                "capabilityVersion": "2"
            }
        ]);
        let skills = skills_from_providers(&providers);
        assert_eq!(skills.len(), 2);
        assert_eq!(skills[0]["id"], "echo");
        assert_eq!(skills[1]["id"], "llm");

        let card = build_agent_card(
            &CardConfig {
                name: "kcell-a2a",
                description: "test",
                url: "http://127.0.0.1:3457",
                version: "0.1.0",
            },
            &providers,
        );
        assert_eq!(card["protocolVersion"], "0.3.0");
        assert_eq!(card["capabilities"]["streaming"], false);
        assert_eq!(card["skills"].as_array().unwrap().len(), 2);
    }
}
