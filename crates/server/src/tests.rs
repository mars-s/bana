use bana_commands::Session;
use bana_core::{Date, sample};
use serde_json::json;

use super::*;

fn host() -> Host {
    let today = Date::new(2026, 10, 10).unwrap();
    let mut s = Session::with_today(sample::sample(today).unwrap(), today);
    s.run("nav.today", &json!({})).unwrap();
    Host::new(s)
}

#[test]
fn page_lists_rows_and_typing_reaches_the_graph() {
    let mut h = host();
    let p = h.handle("page", &json!({}));
    assert!(p["title"].as_str().unwrap().starts_with("Today"));
    let rows = p["rows"]["items"].as_array().unwrap();
    assert!(!rows.is_empty());
    let first = rows.iter().find(|r| r["k"] == "node").unwrap()["id"].as_u64().unwrap();
    let r = h.handle("run", &json!({"command": "node.setName", "params": {"node": first, "name": "typed", "caret": 5}}));
    assert!(r.get("error").is_none(), "{r}");
    assert_eq!(h.session.graph.name(bana_core::NodeId(first)), "typed");
}

#[test]
fn a_logged_edit_sent_twice_runs_once() {
    // A reload replays the write-ahead log; the request in flight (or the unload beacon) may
    // arrive too. The same (client, seq) runs once; a new page's numbers are its own.
    let mut h = host();
    let parent = h.session.zoom.0;
    let add = |seq: u64, client: &str| json!({"command": "node.add", "params": {"parent": parent, "name": "once"}, "wal": [client, seq]});
    let count = |h: &Host| h.session.graph.children(bana_core::NodeId(parent)).iter().filter(|c| h.session.graph.name(**c) == "once").count();
    assert!(h.handle("run", &add(1, "a")).get("dup").is_none());
    assert_eq!(h.handle("run", &add(1, "a"))["dup"], json!(true));
    h.handle("run", &add(2, "a"));
    assert_eq!(h.handle("run", &add(1, "a"))["dup"], json!(true), "an older one late");
    h.handle("run", &add(1, "b"));
    assert_eq!(count(&h), 3);
}

#[test]
fn panels_keep_their_own_page() {
    let mut h = host();
    let today = h.session.zoom;
    h.handle("panel.open", &json!({"name": "p2", "node": h.session.graph.sys.library.0}));
    let p2 = h.handle("page", &json!({"panel": "p2"}));
    assert_eq!(p2["zoom"], json!(h.session.graph.sys.library.0));
    let main = h.handle("page", &json!({"panel": MAIN_PANEL}));
    assert_eq!(main["zoom"], json!(today.0));
}

#[test]
fn windows_are_clamped_and_bad_input_is_an_error() {
    let mut h = host();
    let p = h.handle("page", &json!({"start": 1_000_000, "count": 99_999_999}));
    assert_eq!(p["rows"]["items"].as_array().unwrap().len(), 0);
    assert!(h.handle("run", &json!({"command": "nope"}))["error"].is_string());
    assert!(h.handle("bogus", &json!({}))["error"].is_string());
    assert!(h.handle("sidebar", &json!({}))["pinned"].is_array());
}

#[test]
fn origins_from_other_sites_are_refused() {
    assert!(http_origin_ok(None));
    assert!(http_origin_ok(Some("http://127.0.0.1:5173")));
    assert!(http_origin_ok(Some("http://localhost:8787")));
    assert!(http_origin_ok(Some("tauri://localhost")));
    assert!(!http_origin_ok(Some("https://evil.example")));
    assert!(!http_origin_ok(Some("null")));
    assert!(!http_origin_ok(Some("http://localhost.evil.example")));
}

fn http_origin_ok(o: Option<&str>) -> bool {
    crate::http::origin_ok(o)
}

#[test]
fn a_field_page_reads_schema_its_supertag_and_itself() {
    let mut h = host();
    let status = h.session.graph.find_field("Status").unwrap();
    h.handle("run", &json!({"command": "nav.open", "params": {"node": status.0}}));
    let p = h.handle("page", &json!({}));
    let names: Vec<&str> = p["crumbs"].as_array().unwrap().iter().map(|c| c["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["Schema", "#task", "Status"]);
    assert_eq!(p["fieldType"], json!("options"));
}

#[test]
fn lines_typed_in_two_windows_rebase_over_each_other() {
    // Both insert into "Volunteer meetup": A at the end, B at the start, from the same text.
    assert_eq!(rebase("Volunteer meetup", "Volunteer meetup AAA", "BBB Volunteer meetup"), ("BBB Volunteer meetup AAA".into(), 4));
    assert_eq!(rebase("Volunteer meetup", "BBB Volunteer meetup", "Volunteer meetup AAA"), ("BBB Volunteer meetup AAA".into(), 0));
    // A deletion before this window's insert shifts it back.
    assert_eq!(rebase("abcdef", "abcdefX", "cdef"), ("cdefX".into(), -2));
    // Both at the same place: theirs first.
    assert_eq!(rebase("ab", "aXb", "aYb"), ("aYXb".into(), 1));
    // Overlapping changes: this window's text wins.
    assert_eq!(rebase("abcdef", "aZf", "abYf"), ("aZf".into(), 0));
}

#[test]
fn each_window_has_its_own_page_and_caret() {
    let mut h = host();
    let rows = h.handle("page", &json!({"window": "A"}))["rows"]["items"].clone();
    let ids: Vec<u64> = rows.as_array().unwrap().iter().filter(|r| r["k"] == "node").map(|r| r["id"].as_u64().unwrap()).collect();
    let (one, two) = (ids[0], ids[1]);
    let name = |h: &Host, n: u64| h.session.graph.name(bana_core::NodeId(n)).to_string();
    let (n1, n2) = (name(&h, one), name(&h, two));
    h.handle("run", &json!({"window": "A", "command": "focus.set", "params": {"node": one, "caret": 0}}));
    h.handle("run", &json!({"window": "B", "command": "focus.set", "params": {"node": two, "caret": 0}}));
    assert_eq!(h.handle("page", &json!({"window": "A"}))["focus"], json!(one));
    assert_eq!(h.handle("page", &json!({"window": "B"}))["focus"], json!(two), "B's caret stays in B's line");
    // Both type in one line at once, each from the text it had.
    h.handle("run", &json!({"window": "A", "command": "node.setName", "params": {"node": one, "name": format!("{n1} AAA"), "base": n1}}));
    h.handle("run", &json!({"window": "B", "command": "node.setName", "params": {"node": one, "name": format!("BBB {n1}"), "base": n1}}));
    assert_eq!(name(&h, one), format!("BBB {n1} AAA"));
    assert_eq!(name(&h, two), n2);
    // Panels B opens are its own too.
    h.handle("panel.open", &json!({"window": "B", "name": "p2", "node": one}));
    h.handle("panel.open", &json!({"window": "A", "name": "p2", "node": two}));
    assert_eq!(h.handle("page", &json!({"window": "B", "panel": "p2"}))["zoom"], json!(one));
    assert_eq!(h.handle("page", &json!({"window": "A", "panel": "p2"}))["zoom"], json!(two));
}

#[test]
fn pickers_show_descriptions_recents_and_the_slash_set() {
    let mut h = host();
    let tags = h.handle("choices", &json!({"mode": "tag", "query": "task"}));
    let t = &tags["items"][0];
    assert_eq!(t["label"], json!("task"));
    assert!(t["workspace"].is_string() && t["description"].is_string());
    // "@" before anything is typed: recently opened nodes, with their paths.
    let lib = h.session.graph.sys.library;
    h.handle("run", &json!({"command": "nav.open", "params": {"node": lib.0}}));
    h.handle("run", &json!({"command": "nav.today", "params": {}}));
    let refs = h.handle("choices", &json!({"mode": "reference", "query": ""}));
    assert!(!refs["items"].as_array().unwrap().is_empty());
    assert!(refs["items"][0]["path"].is_string());
    let slash: Vec<String> = h.handle("choices", &json!({"mode": "slash", "query": ""}))["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["label"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(slash, ["Paste", "Search node", "Field", "Reference", "Heading", "Checkbox", "Checklist", "More commands"]);
    // `:` and a word (UC-06.38): emoji by any word of their name, never a "create" row.
    let emoji = h.handle("choices", &json!({"mode": "emoji", "query": "rock"}));
    assert_eq!(emoji["items"][0]["emoji"], "🚀");
    assert_eq!(h.handle("choices", &json!({"mode": "emoji", "query": "zzz"}))["items"].as_array().map(Vec::len), Some(0));
}

#[test]
fn a_field_name_typed_after_a_line_stays_below_it_when_committed() {
    // DIFFS N76: "x", Enter, `>`, "Fresh9", reload: the field row stays after "x".
    let mut h = host();
    let page = h.handle("run", &json!({"command": "node.add", "params": {"name": "N76"}}))["result"]["id"].as_u64().unwrap();
    let x = h.handle("run", &json!({"command": "node.add", "params": {"name": "x", "parent": page}}))["result"]["id"].as_u64().unwrap();
    let line = h.handle("run", &json!({"command": "node.add", "params": {"name": "", "parent": page}}))["result"]["id"].as_u64().unwrap();
    h.handle("fieldDraft", &json!({"owner": page, "line": line, "name": "Fresh9"}));
    assert_eq!(h.handle("fieldDraft", &json!({"commit": true}))["ok"], json!(true));
    let g = &h.session.graph;
    let f = g.find_field("Fresh9").unwrap();
    let entry = g.get(bana_core::NodeId(page)).unwrap().fields.iter().find(|e| e.field == f).unwrap();
    assert_eq!(entry.after, Some(bana_core::NodeId(x)));
    assert!(!g.children(bana_core::NodeId(page)).contains(&bana_core::NodeId(line)));
}

#[test]
fn keys_typed_at_the_end_of_a_line_another_window_splits_follow_the_end() {
    // DIFFS N74: B's caret is at the end of "Pick a date"; A presses Enter after "Pick".
    let mut h = host();
    h.handle("page", &json!({"window": "A"}));
    let page = h.session.zoom.0;
    let line = h.handle("run", &json!({"window": "A", "command": "node.add", "params": {"name": "Pick a date", "parent": page}}))["result"]["id"]
        .as_u64()
        .unwrap();
    let name = |h: &Host, n: u64| h.session.graph.name(bana_core::NodeId(n)).to_string();
    h.handle("run", &json!({"window": "B", "command": "focus.set", "params": {"node": line, "caret": 11}}));
    h.handle("run", &json!({"window": "A", "command": "node.enter", "params": {"node": line, "caret": 4}}));
    let second = h.handle("page", &json!({"window": "A"}))["focus"].as_u64().unwrap();
    assert_eq!((name(&h, line), name(&h, second)), ("Pick".to_string(), " a date".to_string()));
    let b = h.handle("page", &json!({"window": "B"}));
    assert_eq!((b["focus"].as_u64(), b["caret"].as_u64()), (Some(second), Some(7)), "B's caret moved with the end");
    // B's keys sent before it saw the split (from the old text) go on at the end too.
    h.handle(
        "run",
        &json!({"window": "B", "command": "node.setName", "params": {"node": line, "name": "Pick a date123", "base": "Pick a date", "caret": 14}}),
    );
    assert_eq!((name(&h, line), name(&h, second)), ("Pick".to_string(), " a date123".to_string()));
    // A caret before the split point stays in the first half.
    h.handle(
        "run",
        &json!({"window": "B", "command": "node.setName", "params": {"node": line, "name": "XPick a date", "base": "Pick a date", "caret": 1}}),
    );
    assert_eq!(name(&h, line), "XPick");
    // Enter at the end of a line moves no text: later typing in it stays there.
    h.handle("run", &json!({"window": "A", "command": "node.enter", "params": {"node": line, "caret": 5}}));
    h.handle("run", &json!({"window": "B", "command": "node.setName", "params": {"node": line, "name": "XPick!", "base": "XPick", "caret": 6}}));
    assert_eq!(name(&h, line), "XPick!");
}

/// One HTTP request to a local test server: (status line, body).
fn http(port: u16, method: &str, path: &str, headers: &str, body: &str) -> (String, String) {
    use std::io::{Read, Write};
    let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    write!(s, "{method} {path} HTTP/1.1\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).unwrap();
    let (head, body) = out.split_once("\r\n\r\n").unwrap();
    (head.lines().next().unwrap().to_string(), body.to_string())
}

#[test]
fn mcp_over_streamable_http_needs_the_token_and_runs_the_tools() {
    // The app's own MCP endpoint (POST /mcp), the same tools as stdio; also through a tunnel.
    let token = "k".repeat(32);
    let shared = http::Shared::with_token(host(), token.clone());
    let listener = http::bind(0).unwrap();
    let port = listener.local_addr().unwrap().port();
    let s2 = std::sync::Arc::clone(&shared);
    std::thread::spawn(move || http::serve_on(s2, listener, std::env::temp_dir()));
    let auth = format!("Authorization: Bearer {token}\r\nContent-Type: application/json\r\n");
    let init = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}"#;
    assert!(http(port, "POST", "/mcp", "", init).0.contains("401"));
    assert!(http(port, "POST", "/mcp", "Authorization: Bearer nope\r\n", init).0.contains("401"));
    let (status, body) = http(port, "POST", "/mcp", &auth, init);
    assert!(status.contains("200"), "{status}");
    assert_eq!(serde_json::from_str::<Value>(&body).unwrap()["result"]["serverInfo"]["name"], "bana");
    let (status, _) = http(port, "POST", "/mcp", &auth, r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#);
    assert!(status.contains("202"), "{status}");
    // Through a tunnel the Host is a public name: the token still lets it in.
    let tunnel = format!("Host: bana.example.org\r\n{auth}");
    let call = r#"[{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"add_node","arguments":{"name":"From the cloud"}}},{"jsonrpc":"2.0","id":3,"method":"tools/list"}]"#;
    let (status, body) = http(port, "POST", "/mcp", &tunnel, call);
    assert!(status.contains("200"), "{status}");
    let v: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v[0]["result"]["isError"], false, "{v}");
    assert!(v[1]["result"]["tools"].as_array().unwrap().len() >= 6);
    let id = bana_core::NodeId(v[0]["result"]["structuredContent"]["id"].as_u64().unwrap());
    assert_eq!(shared.host.lock().unwrap().session.graph.name(id), "From the cloud");
    // A page on another origin is refused; GET has no stream.
    assert!(http(port, "POST", "/mcp", &format!("Origin: https://evil.example\r\n{auth}"), init).0.contains("403"));
    assert!(http(port, "GET", "/mcp", &auth, "").0.contains("405"));
    // The rest of the API still refuses a foreign Host (DNS rebinding).
    assert!(http(port, "POST", "/rpc", &tunnel, r#"{"method":"version"}"#).0.contains("403"));
}

#[test]
fn a_sample_for_a_new_file_is_written_at_once() {
    let dir = std::env::temp_dir().join(format!("bana-sample-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("graph.bana");
    let _ = std::fs::remove_file(&file);
    open_session(Some(file.clone()), true, Date::new(2026, 10, 10)).unwrap();
    assert!(file.exists(), "the sample is on disk before any edit");
    let g = bana_core::Graph::load(&file).unwrap();
    assert!(g.find_tag("task").is_some());
}

#[test]
fn an_answered_edit_survives_a_crash_before_the_next_save() {
    // DIFFS N77: the page drops an edit from its log when the engine answers, so the answer must
    // mean "on disk". Here the process "dies" (the host is dropped, no save) right after answers.
    let dir = std::env::temp_dir().join(format!("bana-n77-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("graph.bana");
    let today = Date::new(2026, 10, 10).unwrap();
    let mut s = Session::with_today(sample::sample(today).unwrap(), today);
    s.graph.save(&path).unwrap();
    s.path = Some(path.clone());
    let mut h = Host::new(s);
    let lib = h.session.graph.sys.library.0;
    let r = h.handle("run", &json!({"command": "node.add", "params": {"name": "kill line", "parent": lib}, "wal": ["c", 1]}));
    let id = r["result"]["id"].as_u64().unwrap();
    for (i, name) in ["kill line three", "kill line three A"].iter().enumerate() {
        let r = h.handle("run", &json!({"command": "node.setName", "params": {"node": id, "name": name}, "wal": ["c", 2 + i]}));
        assert!(r.get("error").is_none(), "{r}");
    }
    drop(h);
    let g = bana_core::Graph::load(&path).unwrap();
    assert_eq!(g.name(bana_core::NodeId(id)), "kill line three A");
    // The answer to the last edit was lost with the process; the page sends it again to the
    // restarted engine, which knows it already ran.
    let mut again = Host::new(Session::with_today(g.clone(), today));
    let r = again.handle("run", &json!({"command": "node.add", "params": {"name": "kill line", "parent": lib}, "wal": ["c", 1]}));
    assert_eq!(r["dup"], true, "{r}");
    // A full save takes the log in; loading again gives the same graph.
    let mut g2 = g.clone();
    g2.save(&path).unwrap();
    assert!(!bana_core::graph::log_path(&path).exists());
    assert_eq!(bana_core::Graph::load(&path).unwrap().name(bana_core::NodeId(id)), "kill line three A");
    // A frame cut short by a crash mid-write is ignored, the frames before it kept.
    let mut s = Session::with_today(bana_core::Graph::load(&path).unwrap(), today);
    s.path = Some(path.clone());
    let mut h = Host::new(s);
    h.handle("run", &json!({"command": "node.setName", "params": {"node": id, "name": "after save"}}));
    drop(h);
    let log = bana_core::graph::log_path(&path);
    let mut bytes = std::fs::read(&log).unwrap();
    bytes.extend_from_slice(&[200, 0, 0, 0, 1, 2]);
    std::fs::write(&log, bytes).unwrap();
    assert_eq!(bana_core::Graph::load(&path).unwrap().name(bana_core::NodeId(id)), "after save");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn assistants_over_http_mcp_meet_the_agent_policy_and_are_named() {
    // Agent permission tiers: what comes in at /mcp (a local tool, a gateway, the remote connector)
    // is checked; a refusal leaves the files byte for byte; edits carry the connector's name. The
    // person's own windows are not limited.
    let dir = std::env::temp_dir().join(format!("bana-http-agents-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("graph.bana");
    let _ = std::fs::remove_file(&file);
    let mut h = host();
    h.session.path = Some(file.clone());
    h.agents_file = dir.join("agents.json");
    h.save().unwrap();
    let token = "a".repeat(32);
    let shared = http::Shared::with_token(h, token.clone());
    let listener = http::bind(0).unwrap();
    let port = listener.local_addr().unwrap().port();
    let s2 = std::sync::Arc::clone(&shared);
    std::thread::spawn(move || http::serve_on(s2, listener, std::env::temp_dir()));
    let snapshot = || {
        let mut all: Vec<(String, Vec<u8>)> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| (e.file_name().to_string_lossy().into_owned(), std::fs::read(e.path()).unwrap_or_default()))
            .collect();
        all.sort();
        all
    };
    let before = snapshot();
    let auth = format!("Authorization: Bearer {token}\r\nContent-Type: application/json\r\nX-Bana-Agent: lifeos\r\n");
    let exec = |command: &str, params: Value| {
        let msg = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "execute", "arguments": {"command": command, "params": params}}});
        let (_, body) = http(port, "POST", "/mcp", &auth, &msg.to_string());
        serde_json::from_str::<Value>(&body).unwrap()["result"].clone()
    };
    for command in ["trash.empty", "node.hardDelete", "workspace.create", "commandNode.on", "settings.set", "file.save", "import.folder"] {
        let r = exec(command, json!({"name": "x", "path": dir.join("x").to_string_lossy()}));
        assert_eq!(r["isError"], true, "{command}: {r}");
        assert!(r["content"][0]["text"].as_str().unwrap().starts_with("refused:"), "{r}");
    }
    assert_eq!(snapshot(), before, "the graph file and its edit log are untouched");
    let r = exec("node.add", json!({"name": "Captured"}));
    assert_eq!(r["isError"], false, "{r}");
    let id = bana_core::NodeId(r["structuredContent"]["id"].as_u64().unwrap());
    assert_eq!(shared.host.lock().unwrap().session.graph.get(id).unwrap().by.as_deref(), Some("lifeos"));
    // Without the header, the name from initialize in the same request.
    let batch = r#"[{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"clientInfo":{"name":"ChatGPT"}}},{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"add_node","arguments":{"name":"Asked"}}}]"#;
    let plain = format!("Authorization: Bearer {token}\r\nContent-Type: application/json\r\n");
    let (_, body) = http(port, "POST", "/mcp", &plain, batch);
    let v: Value = serde_json::from_str(&body).unwrap();
    let asked = bana_core::NodeId(v[1]["result"]["structuredContent"]["id"].as_u64().unwrap());
    assert_eq!(shared.host.lock().unwrap().session.graph.get(asked).unwrap().by.as_deref(), Some("ChatGPT"));
    // The app's own pages: no policy, no name.
    let mut host = shared.host.lock().unwrap();
    assert!(host.handle("run", &json!({"command": "trash.empty", "params": {}})).get("error").is_none());
    let r = host.handle("run", &json!({"window": "w1", "command": "node.add", "params": {"name": "Typed"}}));
    let typed = bana_core::NodeId(r["result"]["id"].as_u64().unwrap());
    assert_eq!(host.session.graph.get(typed).unwrap().by, None);
}

#[test]
fn a_scoped_agent_token_fixes_who_is_asking_and_what_it_may_do() {
    let dir = std::env::temp_dir().join(format!("bana-http-scoped-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut h = host();
    h.agents_file = dir.join("agents.json");
    // agents.json loosens things for a client named "sync"; the scoped token must not get that.
    std::fs::write(&h.agents_file, r#"{"allow": ["trash.empty"], "clients": {"sync": ["node.toggleLock"]}}"#).unwrap();
    let tokens = dir.join(bana_commands::agents::TOKENS);
    let scoped = bana_commands::agents::create_token(&tokens, "sync", &["mirror.*".into()]).unwrap();
    let parent = h.session.zoom;
    let general = "g".repeat(32);
    let shared = http::Shared::with_token(h, general.clone());
    let listener = http::bind(0).unwrap();
    let port = listener.local_addr().unwrap().port();
    let s2 = std::sync::Arc::clone(&shared);
    std::thread::spawn(move || http::serve_on(s2, listener, std::env::temp_dir()));
    let exec = |token: &str, command: &str, params: Value| {
        let auth = format!("Authorization: Bearer {token}\r\nContent-Type: application/json\r\nX-Bana-Agent: claude\r\n");
        let msg = json!([
            {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"clientInfo": {"name": "someone-else"}}},
            {"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "execute", "arguments": {"command": command, "params": params}}}
        ]);
        let (status, body) = http(port, "POST", "/mcp", &auth, &msg.to_string());
        assert!(status.contains("200"), "{status}");
        serde_json::from_str::<Value>(&body).unwrap()[1]["result"].clone()
    };
    let upsert = json!({"tag": "thing", "key": "Things ID", "id": "T-1", "parent": parent.0, "name": "Plant garlic", "fields": {"Status": "open"}});
    // The token's name, whatever the header and clientInfo say.
    let r = exec(&scoped, "mirror.upsert", upsert.clone());
    assert_eq!(r["isError"], false, "{r}");
    let n = bana_core::NodeId(r["structuredContent"]["node"].as_u64().unwrap());
    {
        let host = shared.host.lock().unwrap();
        assert_eq!(host.session.graph.get(n).unwrap().by.as_deref(), Some("sync"));
        assert!(host.session.graph.locked(n));
    }
    // Only its own allowances: not agents.json's (global or for its name).
    for command in ["trash.empty", "node.toggleLock"] {
        let r = exec(&scoped, command, json!({"node": n.0}));
        assert!(r["content"][0]["text"].as_str().unwrap().starts_with("refused:"), "{command}: {r}");
    }
    // The general token keeps today's policy, and mirror.* is not for it; a client calling itself
    // "sync" is recorded as unverified.
    let r = exec(&general, "mirror.upsert", upsert);
    assert!(r["content"][0]["text"].as_str().unwrap().starts_with("refused:"), "{r}");
    let r = exec(&general, "node.add", json!({"name": "Hello"}));
    let hello = bana_core::NodeId(r["structuredContent"]["id"].as_u64().unwrap());
    assert_eq!(shared.host.lock().unwrap().session.graph.get(hello).unwrap().by.as_deref(), Some("claude"));
    let auth = format!("Authorization: Bearer {general}\r\nContent-Type: application/json\r\nX-Bana-Agent: sync\r\n");
    let msg = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "add_node", "arguments": {"name": "Spoofed"}}});
    let (_, body) = http(port, "POST", "/mcp", &auth, &msg.to_string());
    let spoofed = bana_core::NodeId(serde_json::from_str::<Value>(&body).unwrap()["result"]["structuredContent"]["id"].as_u64().unwrap());
    assert_eq!(shared.host.lock().unwrap().session.graph.get(spoofed).unwrap().by.as_deref(), Some("sync (unverified)"));
    // A scoped token opens /mcp only: the JSON API refuses it, and an unknown token gets 401.
    let rpc = format!("Authorization: Bearer {scoped}\r\nContent-Type: application/json\r\n");
    assert!(http(port, "POST", "/rpc", &rpc, r#"{"method":"run","params":{"command":"node.toggleLock","params":{}}}"#).0.contains("401"));
    assert!(http(port, "POST", "/mcp", &format!("Authorization: Bearer {}\r\n", "x".repeat(64)), "{}").0.contains("401"));
    let _ = std::fs::remove_dir_all(&dir);
}
