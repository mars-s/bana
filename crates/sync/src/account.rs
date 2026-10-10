//! Bana ID on a device (docs/bana-id.md): `id.*` requests from the profile page and onboarding.
//!
//! Signing in happens on a page the relay serves (passkeys are tied to the relay's host). The app
//! opens it ([`Service::id_begin`]) with a fresh X25519 key; the page hands a one-time code back,
//! to a one-shot listener on 127.0.0.1 (desktop) or through `bana://auth/callback` (iPhone,
//! [`Service::id_finish`]). The code is exchanged once; the passkey's PRF output, masked for this
//! app only, unlocks the workspace key the relay keeps wrapped ([`crate::id`]).

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::config::IdSession;
use crate::crypto::{Kind, WorkspaceKey};
use crate::id::{Ephemeral, RecoveryCode, WrapKind, prf_wrapping_key, unwrap, wrap};
use crate::net::Service;
use crate::{Error, Result, b64};

/// The relay Bana ID uses unless the person types another: the one this project's owner deployed
/// (`cloudflare/sync/wrangler.jsonc` names the same host as the passkeys' relying party).
pub const DEFAULT_RELAY: &str = "https://bana-sync.lifeos-read-mirror-worker.workers.dev";

/// How long a sign-in page may take before the app stops waiting.
const FLOW_FOR: Duration = Duration::from_secs(10 * 60);

/// A sign-in in progress: what came back must match it.
pub(crate) struct Flow {
    mode: String,
    relay: String,
    name: String,
    state: String,
    eph: Ephemeral,
    started: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Mode {
    Create,
    SignIn,
    Add,
}

fn mode_of(s: &str) -> Result<Mode> {
    match s {
        "create" => Ok(Mode::Create),
        "signin" => Ok(Mode::SignIn),
        "add" => Ok(Mode::Add),
        _ => Err(Error::Config("mode: create, signin or add".into())),
    }
}

/// Percent-encodes a query value.
fn pct(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

fn unpct(s: &str) -> String {
    let hex = |c: u8| (c as char).to_digit(16);
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        match (b[i], b.get(i + 1).and_then(|c| hex(*c)), b.get(i + 2).and_then(|c| hex(*c))) {
            (b'%', Some(h), Some(l)) => {
                out.push((h * 16 + l) as u8);
                i += 3;
            }
            (c, _, _) => {
                out.push(if c == b'+' { b' ' } else { c });
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The query parameters of a callback address.
fn query(url: &str) -> Vec<(String, String)> {
    let q = url.split_once('?').map(|(_, q)| q).unwrap_or_default();
    let q = q.split('#').next().unwrap_or_default();
    q.split('&').filter_map(|kv| kv.split_once('=')).map(|(k, v)| (unpct(k), unpct(v))).collect()
}

/// One call to the relay's Bana ID API.
fn api(relay: &str, method: &str, path: &str, body: Option<&Value>, token: Option<&str>) -> Result<Value> {
    let url = format!("{}/id/api/{path}", relay.trim_end_matches('/'));
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(20))).http_status_as_error(false).build().into();
    let auth = token.map(|t| format!("Bearer {t}"));
    let res = match (method, body) {
        ("GET", _) | ("DELETE", None) => {
            let mut r = if method == "GET" { agent.get(&url) } else { agent.delete(&url) }.header("accept", "application/json");
            if let Some(a) = &auth {
                r = r.header("authorization", a);
            }
            r.call()
        }
        (_, b) => {
            let body = b.cloned().unwrap_or_else(|| json!({}));
            let mut r = match method {
                "PUT" => agent.put(&url),
                "PATCH" => agent.patch(&url),
                _ => agent.post(&url),
            }
            .header("content-type", "application/json");
            if let Some(a) = &auth {
                r = r.header("authorization", a);
            }
            r.send(body.to_string().as_bytes())
        }
    };
    let mut res = res.map_err(|e| Error::Net(format!("the relay cannot be reached: {e}")))?;
    let status = res.status().as_u16();
    let text = res.body_mut().read_to_string().unwrap_or_default();
    let v: Value = serde_json::from_str(&text).unwrap_or_else(|_| json!({"error": format!("the relay answered {status}")}));
    if status >= 400 || v.get("error").is_some() {
        let msg = v.get("error").and_then(Value::as_str).unwrap_or("the relay refused").to_string();
        return Err(if status == 401 { Error::Config(format!("signed out on the relay: {msg}")) } else { Error::Net(msg) });
    }
    Ok(v)
}

fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or_default()
}

/// Opens `url` in the default browser (the desktop engine; the phone uses its own sign-in sheet).
fn open_browser(url: &str) -> Result<()> {
    let cmd = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(windows) {
        "explorer"
    } else {
        "xdg-open"
    };
    std::process::Command::new(cmd).arg(url).spawn().map(|_| ()).map_err(|e| Error::Config(format!("cannot open the browser: {e}")))
}

const DONE_PAGE: &str = "<!doctype html><meta charset=utf-8><meta name=viewport content='width=device-width'><title>Bana</title><body style='font:15px system-ui;display:grid;place-items:center;min-height:90vh;background:#1a1c1e;color:#cdcdcd'><p>%MSG% You can close this tab and go back to Bana.</p>";

impl Service {
    /// Starts a sign-in (`create` a Bana ID, `signin` on this device, `add` a passkey): answers
    /// the page's address. `ret`: `loopback` (this computer listens once on 127.0.0.1) or `bana`
    /// (the iPhone's sign-in sheet catches `bana://auth/callback`).
    pub fn id_begin(&self, params: &Value) -> Result<Value> {
        let mode = mode_of(s(params, "mode"))?;
        let (cfg_relay, signed_in) = {
            let st = self.lock();
            let c = st.client.as_ref();
            (c.map(|c| c.cfg.relay.clone()), c.and_then(|c| c.cfg.id.clone()))
        };
        let typed = Some(s(params, "relay").trim()).filter(|r| !r.is_empty()).map(str::to_string);
        let relay = match (mode, &cfg_relay) {
            (Mode::SignIn, Some(_)) if signed_in.is_some() => return Err(Error::Config("this device is already signed in".into())),
            (Mode::SignIn, Some(_)) => {
                return Err(Error::Config(
                    "sync is already on here with a pairing code; turn it off first (signing in brings your synced notes to this device)".into(),
                ));
            }
            (Mode::Create, Some(_)) if signed_in.is_some() => return Err(Error::Config("this device is already signed in to a Bana ID".into())),
            (Mode::Add, _) if signed_in.is_none() => return Err(Error::Config("sign in first".into())),
            // Creating on a device that already syncs: the Bana ID lives on that relay.
            (_, Some(r)) => r.clone(),
            (_, None) => crate::config::check_relay(typed.as_deref().unwrap_or(DEFAULT_RELAY))?,
        };
        let ticket = match (mode, &signed_in) {
            (Mode::Add, Some(id)) => Some(s(&api(&relay, "POST", "ticket", None, Some(&id.session))?, "ticket").to_string()),
            _ => None,
        };
        let eph = Ephemeral::generate()?;
        let state = crate::id::b64url(&crate::crypto::random::<16>()?);
        let name = Some(s(params, "name").trim()).filter(|n| !n.is_empty()).map(str::to_string).unwrap_or_else(crate::net::default_name);
        let loopback = s(params, "ret") != "bana";
        let listener = if loopback {
            Some(TcpListener::bind(("127.0.0.1", 0)).map_err(|e| Error::Net(format!("cannot listen for the sign-in: {e}")))?)
        } else {
            None
        };
        let ret = match &listener {
            Some(l) => format!("http://127.0.0.1:{}/auth/callback", l.local_addr().map_err(|e| Error::Net(e.to_string()))?.port()),
            None => "bana://auth/callback".to_string(),
        };
        let person = Some(s(params, "person").trim()).filter(|n| !n.is_empty()).unwrap_or("Bana");
        let mut url = format!(
            "{}/id?mode={}&app={}&state={}&ret={}&name={}",
            relay.trim_end_matches('/'),
            s(params, "mode"),
            eph.public(),
            state,
            pct(&ret),
            pct(person)
        );
        if let Some(t) = &ticket {
            url.push_str(&format!("&ticket={}", pct(t)));
        }
        {
            let mut st = self.lock();
            st.flow = Some(Flow { mode: s(params, "mode").into(), relay, name, state: state.clone(), eph, started: Instant::now() });
            st.id_note = None;
        }
        if let Some(l) = listener {
            let me = self.clone();
            std::thread::Builder::new().name("bana-id".into()).spawn(move || me.listen(l, state)).map_err(|e| Error::Net(e.to_string()))?;
        }
        if params.get("open").and_then(Value::as_bool) == Some(true) {
            open_browser(&url)?;
        }
        Ok(json!({"url": url, "ret": ret}))
    }

    /// The desktop's one-shot listener for the page's hand-back.
    fn listen(&self, l: TcpListener, state: String) {
        let _ = l.set_nonblocking(true);
        let deadline = Instant::now() + FLOW_FOR;
        while Instant::now() < deadline {
            if self.lock().flow.as_ref().is_none_or(|f| f.state != state) {
                return;
            }
            let Ok((mut tcp, _)) = l.accept() else {
                std::thread::sleep(Duration::from_millis(100));
                continue;
            };
            let _ = tcp.set_nonblocking(false);
            let _ = tcp.set_read_timeout(Some(Duration::from_secs(5)));
            let mut line = String::new();
            let _ = BufReader::new(&tcp).read_line(&mut line);
            let path = line.split_whitespace().nth(1).unwrap_or_default().to_string();
            if !path.starts_with("/auth/callback?") {
                let _ = tcp.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                continue;
            }
            let msg = match self.id_finish(&path) {
                Ok(_) => "Done.".to_string(),
                Err(e) => format!("That did not work: {e}."),
            };
            let msg = msg.replace('&', "&amp;").replace('<', "&lt;");
            let body = DONE_PAGE.replace("%MSG%", &msg);
            let _ = write!(
                tcp,
                "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nCache-Control: no-store\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            return;
        }
        let mut st = self.lock();
        if st.flow.as_ref().is_some_and(|f| f.state == state) {
            st.flow = None;
            st.id_note = Some("The sign-in page took too long; try again.".into());
        }
    }

    /// The page came back (`.../auth/callback?code=..&state=..`): exchanges the code and finishes
    /// creating, signing in or adding the passkey.
    pub fn id_finish(&self, url: &str) -> Result<Value> {
        let q = query(url);
        let get = |k: &str| q.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone()).unwrap_or_default();
        let flow = {
            let mut st = self.lock();
            match st.flow.take() {
                Some(f) if f.state == get("state") && f.started.elapsed() < FLOW_FOR => f,
                Some(f) => {
                    st.flow = Some(f);
                    return Err(Error::Config("this sign-in does not belong to the one this app started".into()));
                }
                None => return Err(Error::Config("no sign-in was started here (or it already finished)".into())),
            }
        };
        let r = self.finish_flow(&flow, &get("code"));
        let mut st = self.lock();
        st.id_note = Some(match &r {
            Ok(_) => "ok".into(),
            Err(e) => e.to_string(),
        });
        drop(st);
        self.inner.cv.notify_all();
        r.map(|_| self.id_status())
    }

    fn finish_flow(&self, flow: &Flow, code: &str) -> Result<()> {
        if code.is_empty() {
            return Err(Error::Config("the page sent no code".into()));
        }
        let ex = api(&flow.relay, "POST", "exchange", Some(&json!({"code": code})), None)?;
        let prf = match (ex.get("page").and_then(Value::as_str), ex.get("masked").and_then(Value::as_str)) {
            (Some(p), Some(m)) => Some(flow.eph.unmask(p, m)?),
            _ => None,
        };
        let account = s(&ex, "account").to_string();
        let credential = s(&ex, "credential").to_string();
        match mode_of(&flow.mode)? {
            Mode::Create => {
                let id = IdSession { account: account.clone(), session: s(&ex, "session").into(), sid: s(&ex, "sid").into() };
                if self.lock().client.is_none() {
                    self.setup(&flow.relay, &flow.name)?;
                }
                let (key, room, device) = self.keys()?;
                let mut vault = json!({"room": room});
                if let Some(prf) = &prf {
                    vault["wrap"] = json!({"credential": credential, "blob": wrap(&prf_wrapping_key(prf)?, &account, WrapKind::Passkey, &key)?});
                }
                let code = RecoveryCode::generate()?;
                vault["recovery"] = json!({"proof": code.proof(), "blob": wrap(&code.wrapping_key(), &account, WrapKind::Recovery, &key)?});
                api(&flow.relay, "PUT", "vault", Some(&vault), Some(&id.session))?;
                self.name_device(&flow.relay, &id, &key, &room, &device, &flow.name)?;
                self.save_id(Some(id))?;
                let mut st = self.lock();
                st.recovery = Some(code.display());
                st.id_prf = prf.is_some();
                Ok(())
            }
            Mode::SignIn => {
                let id = IdSession { account: account.clone(), session: s(&ex, "session").into(), sid: s(&ex, "sid").into() };
                let key = (|| {
                    let prf = prf.ok_or_else(|| Error::Config("this passkey cannot unlock your notes here (the browser gave no PRF output). Use your recovery code, or pair with a code from another device".into()))?;
                    let blob = ex.get("wrap").and_then(Value::as_str).ok_or_else(|| {
                        Error::Config(
                            "this passkey was never set up to unlock your notes. Use your recovery code, or pair with a code from another device"
                                .into(),
                        )
                    })?;
                    unwrap(&prf_wrapping_key(&prf)?, &account, WrapKind::Passkey, blob)
                })();
                let key = match key {
                    Ok(k) => k,
                    Err(e) => {
                        let _ = api(&flow.relay, "DELETE", &format!("devices/{}", id.sid), None, Some(&id.session));
                        return Err(e);
                    }
                };
                self.join_with(&flow.relay, &flow.name, &key, &ex, id)?;
                self.lock().id_prf = true;
                Ok(())
            }
            Mode::Add => {
                let id = self.lock().client.as_ref().and_then(|c| c.cfg.id.clone()).ok_or_else(|| Error::Config("sign in first".into()))?;
                let prf = prf.ok_or_else(|| {
                    Error::Config("the new passkey was added, but it cannot unlock your notes on a new device (no PRF on this platform)".into())
                })?;
                let (key, _, _) = self.keys()?;
                let blob = wrap(&prf_wrapping_key(&prf)?, &id.account, WrapKind::Passkey, &key)?;
                api(&flow.relay, "PUT", "vault", Some(&json!({"wrap": {"credential": credential, "blob": blob}})), Some(&id.session))?;
                Ok(())
            }
        }
    }

    /// Joins the account's sync room with the invite the relay made at sign-in.
    fn join_with(&self, relay: &str, name: &str, key: &WorkspaceKey, ex: &Value, id: IdSession) -> Result<()> {
        let invite = ex
            .get("invite")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::Config("this Bana ID has no synced notes yet: create it on the device that has your notes".into()))?;
        if s(ex, "room") != key.room() {
            return Err(Error::Crypto("the unlocked key does not match the account's sync room".into()));
        }
        let code = crate::code::Code {
            v: 1,
            relay: relay.to_string(),
            key: key.to_b64(),
            invite: invite.to_string(),
            by: String::new(),
            by_name: "Bana ID".into(),
        };
        self.join(&code.encode(), name)?;
        let (_, room, device) = self.keys()?;
        self.name_device(relay, &id, key, &room, &device, name)?;
        self.save_id(Some(id))
    }

    /// Signs in with the recovery code instead of a passkey (a lost passkey, a changed relay host).
    pub fn id_recover(&self, params: &Value) -> Result<Value> {
        if self.lock().client.is_some() {
            return Err(Error::Config("sync is already on here; turn it off first".into()));
        }
        let relay = crate::config::check_relay(Some(s(params, "relay").trim()).filter(|r| !r.is_empty()).unwrap_or(DEFAULT_RELAY))?;
        let code = RecoveryCode::parse(s(params, "code"))?;
        let name = Some(s(params, "name").trim()).filter(|n| !n.is_empty()).map(str::to_string).unwrap_or_else(crate::net::default_name);
        let ex = api(&relay, "POST", "recover", Some(&json!({"proof": code.proof()})), None)?;
        let account = s(&ex, "account").to_string();
        let id = IdSession { account: account.clone(), session: s(&ex, "session").into(), sid: s(&ex, "sid").into() };
        let key = unwrap(&code.wrapping_key(), &account, WrapKind::Recovery, s(&ex, "recoveryWrap"))?;
        self.join_with(&relay, &name, &key, &ex, id)?;
        Ok(self.id_status())
    }

    /// The workspace key, room and this device's id.
    fn keys(&self) -> Result<(WorkspaceKey, String, String)> {
        let st = self.lock();
        let c = st.client.as_ref().ok_or_else(|| Error::Config("sync is not set up".into()))?;
        Ok((c.cfg.workspace_key()?, c.cfg.room.clone(), c.device_id().to_string()))
    }

    fn save_id(&self, id: Option<IdSession>) -> Result<()> {
        let mut st = self.lock();
        let c = st.client.as_mut().ok_or_else(|| Error::Config("sync is not set up".into()))?;
        c.cfg.id = id;
        c.cfg.save(&self.inner.path)
    }

    fn name_device(&self, relay: &str, id: &IdSession, key: &WorkspaceKey, room: &str, device: &str, name: &str) -> Result<()> {
        let sealed = b64::encode(&key.seal(room, Kind::Name, "bana-id", name.as_bytes())?);
        api(relay, "PATCH", &format!("devices/{}", id.sid), Some(&json!({"name": sealed, "device": device})), Some(&id.session)).map(|_| ())
    }

    fn session(&self) -> Result<(String, IdSession, WorkspaceKey, String)> {
        let st = self.lock();
        let c = st.client.as_ref().ok_or_else(|| Error::Config("not signed in".into()))?;
        let id = c.cfg.id.clone().ok_or_else(|| Error::Config("not signed in".into()))?;
        Ok((c.cfg.relay.clone(), id, c.cfg.workspace_key()?, c.cfg.room.clone()))
    }

    /// What the profile page shows: passkeys and devices (names unsealed here).
    pub fn id_account(&self) -> Result<Value> {
        let (relay, id, key, room) = self.session()?;
        let mut a = api(&relay, "GET", "account", None, Some(&id.session))?;
        if let Some(devs) = a.get_mut("devices").and_then(Value::as_array_mut) {
            for d in devs {
                let name = d
                    .get("name")
                    .and_then(Value::as_str)
                    .and_then(b64::decode)
                    .and_then(|b| key.open(&room, Kind::Name, "bana-id", &b).ok())
                    .map(|b| String::from_utf8_lossy(&b).into_owned())
                    .unwrap_or_else(|| "Unnamed device".into());
                d["name"] = json!(name);
            }
        }
        a["relay"] = json!(relay);
        Ok(a)
    }

    pub fn id_rename_device(&self, sid: &str, name: &str) -> Result<Value> {
        let (relay, id, key, room) = self.session()?;
        let name: String = name.trim().chars().filter(|c| !c.is_control()).take(64).collect();
        if name.is_empty() {
            return Err(Error::Config("a device needs a name".into()));
        }
        let target = IdSession { sid: sid.to_string(), ..id.clone() };
        let sealed = b64::encode(&key.seal(&room, Kind::Name, "bana-id", name.as_bytes())?);
        api(&relay, "PATCH", &format!("devices/{}", pct(&target.sid)), Some(&json!({"name": sealed})), Some(&id.session))?;
        self.id_account()
    }

    pub fn id_remove_device(&self, sid: &str) -> Result<Value> {
        let (relay, id, _, _) = self.session()?;
        if sid == id.sid {
            return Err(Error::Config("this is this device: use Sign out instead".into()));
        }
        api(&relay, "DELETE", &format!("devices/{}", pct(sid)), None, Some(&id.session))?;
        self.id_account()
    }

    pub fn id_remove_passkey(&self, cred: &str) -> Result<Value> {
        let (relay, id, _, _) = self.session()?;
        api(&relay, "DELETE", &format!("credentials/{}", pct(cred)), None, Some(&id.session))?;
        self.id_account()
    }

    /// A new recovery code (the old one stops working); shown once.
    pub fn id_new_recovery(&self) -> Result<Value> {
        let (relay, id, key, _) = self.session()?;
        let code = RecoveryCode::generate()?;
        let blob = wrap(&code.wrapping_key(), &id.account, WrapKind::Recovery, &key)?;
        api(&relay, "PUT", "vault", Some(&json!({"recovery": {"proof": code.proof(), "blob": blob}})), Some(&id.session))?;
        Ok(json!({"recoveryCode": code.display()}))
    }

    /// Signs this device out: the session ends and sync stops here; the notes stay.
    pub fn id_sign_out(&self) -> Result<Value> {
        let (relay, id, _, _) = self.session()?;
        // Signed out on the relay when it can be reached; locally in any case.
        let _ = api(&relay, "DELETE", &format!("devices/{}", pct(&id.sid)), None, Some(&id.session));
        self.leave()?;
        Ok(self.id_status())
    }

    /// Deletes the Bana ID and its sync data from the relay, then stops sync here. Notes stay on
    /// every device.
    pub fn id_delete(&self, confirm: bool) -> Result<Value> {
        if !confirm {
            return Err(Error::Config("deleting a Bana ID needs confirm: true".into()));
        }
        let (relay, id, _, _) = self.session()?;
        api(&relay, "DELETE", "account", None, Some(&id.session))?;
        self.leave()?;
        Ok(self.id_status())
    }

    pub fn id_status(&self) -> Value {
        let st = self.lock();
        let id = st.client.as_ref().and_then(|c| c.cfg.id.as_ref());
        json!({
            "signedIn": id.is_some(),
            "account": id.map(|i| &i.account),
            "relay": st.client.as_ref().map(|c| c.cfg.relay.clone()).unwrap_or_else(|| DEFAULT_RELAY.into()),
            "defaultRelay": DEFAULT_RELAY,
            "syncOn": st.client.is_some(),
            "pending": st.flow.as_ref().map(|f| f.mode.clone()),
            "note": st.id_note,
            "recoveryCode": st.recovery,
            "prf": st.id_prf,
        })
    }

    /// `id.*` requests.
    pub(crate) fn id_handle(&self, method: &str, params: &Value) -> Result<Value> {
        match method {
            "id.status" => Ok(self.id_status()),
            "id.begin" => self.id_begin(params),
            "id.finish" => self.id_finish(s(params, "url")),
            "id.cancel" => {
                self.lock().flow = None;
                Ok(self.id_status())
            }
            "id.recover" => self.id_recover(params),
            "id.account" => self.id_account(),
            "id.renameDevice" => self.id_rename_device(s(params, "sid"), s(params, "name")),
            "id.removeDevice" => self.id_remove_device(s(params, "sid")),
            "id.removePasskey" => self.id_remove_passkey(s(params, "id")),
            "id.newRecoveryCode" => self.id_new_recovery(),
            "id.recoveryShown" => {
                self.lock().recovery = None;
                Ok(self.id_status())
            }
            "id.signOut" => self.id_sign_out(),
            "id.delete" => self.id_delete(params.get("confirm").and_then(Value::as_bool) == Some(true)),
            other => Err(Error::Config(format!("unknown method `{other}`"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn callback_addresses_parse() {
        let q = query("http://127.0.0.1:5/auth/callback?code=ab-c_d&state=x%2By&prf=none");
        assert_eq!(q, vec![("code".into(), "ab-c_d".into()), ("state".into(), "x+y".into()), ("prf".into(), "none".into())]);
        assert_eq!(unpct(&pct("a b/é?&")), "a b/é?&");
        assert_eq!(unpct("%"), "%");
        assert_eq!(unpct("%4"), "%4");
    }
}
