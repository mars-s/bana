//! The person's profile (onboarding and the profile page): their name and avatar, and whether
//! onboarding ran. Kept in `profile.json` beside the graph (the same name is also the graph's
//! #person node, `person.setMe`, so "By" and user fields show it). Nothing here is secret.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::Host;

/// An avatar picture as a `data:image/...;base64,` address, at most this long.
const AVATAR_MAX: usize = 400 * 1024;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    #[serde(default)]
    pub name: String,
    /// A small picture (`data:image/png|jpeg|webp;base64,...`), or none for initials.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar: Option<String>,
    /// Onboarding ran (finished or skipped): it never shows again.
    #[serde(default)]
    pub onboarded: bool,
}

impl Host {
    fn profile_path(&self) -> Option<PathBuf> {
        self.session.path.as_ref().map(|p| p.with_file_name("profile.json"))
    }

    pub fn profile(&self) -> Profile {
        match self.profile_path().map(std::fs::read) {
            Some(Ok(b)) => serde_json::from_slice(&b).unwrap_or_default(),
            // No file to keep it in (a graph in memory): nothing to onboard.
            None => Profile { onboarded: true, ..self.profile_mem.clone() },
            Some(Err(_)) => Profile::default(),
        }
    }

    fn save_profile(&mut self, p: &Profile) -> Result<(), String> {
        let Some(path) = self.profile_path() else {
            self.profile_mem = p.clone();
            return Ok(());
        };
        let body = serde_json::to_vec_pretty(p).map_err(|e| e.to_string())?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, body).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
    }

    fn profile_json(&self) -> Value {
        let p = self.profile();
        json!({"name": p.name, "avatar": p.avatar, "onboarded": p.onboarded, "me": self.session.graph.me().map(|m| m.0)})
    }

    /// `profile.get`, `profile.set {name?, avatar? (null clears), onboarded?}`, and
    /// `onboarding.sample` (the sample notes in place of an empty graph, during onboarding).
    pub(crate) fn profile_request(&mut self, method: &str, params: &Value) -> Value {
        match method {
            "profile.get" => self.profile_json(),
            "profile.set" => {
                let mut p = self.profile();
                if let Some(n) = params.get("name").and_then(Value::as_str) {
                    p.name = n.trim().chars().filter(|c| !c.is_control()).take(80).collect();
                }
                match params.get("avatar") {
                    Some(Value::Null) => p.avatar = None,
                    Some(Value::String(a)) => {
                        let ok = a.len() <= AVATAR_MAX
                            && ["data:image/png;base64,", "data:image/jpeg;base64,", "data:image/webp;base64,"].iter().any(|h| a.starts_with(h));
                        if !ok {
                            return json!({"error": "the picture must be a PNG, JPEG or WebP under 400 KB"});
                        }
                        p.avatar = Some(a.clone());
                    }
                    _ => {}
                }
                if let Some(o) = params.get("onboarded").and_then(Value::as_bool) {
                    p.onboarded = o;
                }
                if let Err(e) = self.save_profile(&p) {
                    return json!({"error": format!("cannot save the profile: {e}")});
                }
                if params.get("name").is_some() && !p.name.is_empty() {
                    let r = self.handle("run", &json!({"command": "person.setMe", "params": {"name": p.name}}));
                    if let Some(e) = r.get("error") {
                        return json!({"error": e});
                    }
                }
                self.version += 1;
                self.profile_json()
            }
            "onboarding.sample" => {
                if self.profile().onboarded {
                    return json!({"error": "the sample replaces the notes; it is offered only during onboarding"});
                }
                match bana_core::sample::sample(self.session.today) {
                    Ok(g) => {
                        self.replace_graph(g);
                        let name = self.profile().name;
                        if !name.is_empty() {
                            let _ = self.session.graph.set_me(&name);
                        }
                        self.journal();
                        self.profile_json()
                    }
                    Err(e) => json!({"error": e.to_string()}),
                }
            }
            other => json!({"error": format!("unknown method `{other}`")}),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_profile_lives_beside_the_graph_and_names_the_person_node() {
        let dir = std::env::temp_dir().join(format!("bana-profile-{}", bana_core::random_hex()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("graph.bana");
        let mut s = bana_commands::Session::new(bana_core::Graph::new());
        s.path = Some(path.clone());
        let mut h = Host::new(s);
        assert_eq!(h.handle("profile.get", &json!({}))["onboarded"], false, "a new install onboards");
        let r = h.handle("profile.set", &json!({"name": "Ada Lovelace", "onboarded": true}));
        assert_eq!(r["name"], "Ada Lovelace");
        let me = bana_core::NodeId(r["me"].as_u64().unwrap());
        assert_eq!(h.session.graph.name(me), "Ada Lovelace");
        assert_eq!(h.session.graph.user_names()[0], "Ada Lovelace", "user fields offer the person first");
        // Renaming keeps the same node.
        let r = h.handle("profile.set", &json!({"name": "Ada"}));
        assert_eq!(r["me"].as_u64(), Some(me.0));
        assert!(h.handle("profile.set", &json!({"avatar": "https://example.com/x.png"}))["error"].is_string());
        assert!(h.handle("profile.set", &json!({"avatar": "data:image/png;base64,AAAA"}))["error"].is_null());
        assert!(dir.join("profile.json").exists());
        assert!(h.handle("onboarding.sample", &json!({}))["error"].is_string(), "only during onboarding");
        std::fs::remove_dir_all(&dir).ok();
    }
}
