use std::process::Command;

fn cli(file: &std::path::Path, args: &[&str]) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_tanacraft-cli")).arg("--file").arg(file).args(args).output().unwrap();
    (out.status.success(), format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)))
}

#[test]
fn add_tag_field_export_round_trip() {
    let dir = std::env::temp_dir().join(format!("tanacraft-cli-test-{}", std::process::id()));
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
