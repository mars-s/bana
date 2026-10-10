use std::process::Command;

fn cli(file: &std::path::Path, args: &[&str]) -> (bool, String) {
    let home = file.parent().unwrap_or(std::path::Path::new("."));
    let out = Command::new(env!("CARGO_BIN_EXE_bana")).env("BANA_HOME", home).arg("--file").arg(file).args(args).output().unwrap();
    (out.status.success(), format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)))
}

#[test]
fn add_tag_field_export_round_trip() {
    let dir = std::env::temp_dir().join(format!("bana-cli-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("g.json");
    let (ok, id) = cli(&f, &["add", "Write", "letter", "--tag", "task"]);
    assert!(ok, "{id}");
    let id = id.trim().to_string();
    let (ok, out) = cli(
        &f,
        &[
            "run",
            "--cmd",
            r#"tag.addField={"tag":"task","field":"Due","type":"date"}"#,
            "--cmd",
            &format!(r#"field.set={{"node":{id},"field":"Due","value":"2026-11-01"}}"#),
        ],
    );
    assert!(ok, "{out}");
    let (ok, md) = cli(&f, &["export"]);
    assert!(ok, "{md}");
    assert_eq!(md.trim(), "- Write letter #task\n  - Due:: 2026-11-01");
    let (ok, hits) = cli(&f, &["search", "letter"]);
    assert!(ok && hits.contains("Write letter"));
    let (ok, id) = cli(&f, &["capture", "Call", "the", "plumber"]);
    assert!(ok, "{id}");
    let (ok, inbox) = cli(&f, &["run", "--cmd", "nav.inbox", "--cmd", "graph.view"]);
    assert!(ok && inbox.contains("Call the plumber"), "{inbox}");
    let md = dir.join("in.md");
    std::fs::write(&md, "- Imported\n  - child\n").unwrap();
    let (ok, out) = cli(&f, &["import", md.to_str().unwrap()]);
    assert!(ok && out.contains("imported 2"), "{out}");
    let (ok, err) = cli(&f, &["run", "--cmd", "no.such"]);
    assert!(!ok && err.contains("unknown command"));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn agent_token_create_writes_a_scoped_token_to_the_data_folder() {
    let dir = std::env::temp_dir().join(format!("bana-cli-agent-token-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("graph.bana");
    let (ok, token) = cli(&f, &["agent-token", "create", "mirror-sync", "--allow", "mirror.*", "--allow", "node.toggleLock"]);
    assert!(ok, "{token}");
    let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(dir.join("agent-tokens.json")).unwrap()).unwrap();
    let entry = &saved["tokens"][0];
    assert_eq!((entry["name"].as_str(), entry["token"].as_str()), (Some("mirror-sync"), Some(token.trim())));
    assert_eq!(entry["allow"], serde_json::json!(["mirror.*", "node.toggleLock"]));
    assert!(!cli(&f, &["agent-token", "create"]).0);
    assert!(!cli(&f, &["agent-token", "list"]).0);
    let _ = std::fs::remove_dir_all(&dir);
}
