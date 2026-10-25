//! Philips Hue: pairing, entertainment areas and the Entertainment stream.
//!
//! - Bridge HTTPS (`call`): the bridge's certificate chains to Signify's
//!   own CA, which no OS trust store has, so `config::tls()` can't talk to
//!   it. These calls go through rustls with a *pinned* certificate instead:
//!   the bridge's SHA-256 fingerprint is taken at pairing (trust on first
//!   use, on the LAN, with the button pressed) and every later call must
//!   match it. Nothing else in the app uses this verifier.
//! - Pairing: `POST /api {"devicetype","generateclientkey":true}` answers
//!   with a username (the API key) and a client key once the bridge's link
//!   button has been pressed.
//! - Streaming (`Stream`): `PUT .../entertainment_configuration/<id>
//!   {"action":"start"}`, then DTLS 1.2 PSK (TLS_PSK_WITH_AES_128_GCM_SHA256)
//!   to UDP 2100. The PSK identity is the *application id* from
//!   `GET /auth/v1` (header `hue-application-id`), not the username; the PSK
//!   is the client key. Packets are "HueStream" v2: a 16-byte header, the
//!   area id (36 ASCII), then 7 bytes a channel (id, R, G, B as u16 BE).
//!   The bridge drops a stream after ~10 s of silence, so frames keep
//!   flowing; `stop` hands the lights back.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use sha2::{Digest, Sha256};

/// Bridge connection details saved in Settings once paired.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct HueBridge {
    pub ip: String,
    pub name: String,
    /// API key ("username") and the entertainment client key (hex).
    pub user: String,
    pub client_key: String,
    /// SHA-256 of the bridge's certificate, hex (pinned at pairing).
    pub cert_sha256: String,
    /// The entertainment area to stream to (id) and its name.
    pub area: String,
    pub area_name: String,
}

// ---- HTTPS with a pinned certificate -------------------------------------

#[derive(Debug)]
struct Pin {
    want: Option<[u8; 32]>,
    seen: Mutex<Option<[u8; 32]>>,
    provider: Arc<rustls::crypto::CryptoProvider>,
}

impl rustls::client::danger::ServerCertVerifier for Pin {
    fn verify_server_cert(
        &self,
        end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        let fp: [u8; 32] = Sha256::digest(end_entity.as_ref()).into();
        *self.seen.lock().unwrap_or_else(|e| e.into_inner()) = Some(fp);
        match self.want {
            Some(w) if w != fp => Err(rustls::Error::General("Hue bridge certificate changed".into())),
            _ => Ok(rustls::client::danger::ServerCertVerified::assertion()),
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.provider.signature_verification_algorithms.supported_schemes()
    }
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    (s.len() % 2 == 0)
        .then(|| (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok()).collect())
        .flatten()
}

/// Quitting: short timeouts, so an unreachable bridge can't hold up exit.
pub static QUITTING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

struct Reply {
    status: u16,
    headers: String,
    body: String,
    cert_sha256: String,
}

/// One HTTPS request to the bridge. `pin` empty = first contact (pairing):
/// accept the certificate and report its fingerprint.
fn call(ip: &str, pin: &str, method: &str, path: &str, key: &str, body: Option<&str>) -> Result<Reply> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let want = if pin.is_empty() {
        None
    } else {
        let v = unhex(pin).filter(|v| v.len() == 32).ok_or_else(|| anyhow!("bad pinned fingerprint"))?;
        Some(<[u8; 32]>::try_from(v.as_slice()).unwrap())
    };
    let verifier = Arc::new(Pin { want, seen: Mutex::new(None), provider: provider.clone() });
    let cfg = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()?
        .dangerous()
        .with_custom_certificate_verifier(verifier.clone())
        .with_no_client_auth();
    let addr: std::net::IpAddr = ip.parse().context("bridge address")?;
    let name = rustls::pki_types::ServerName::IpAddress(addr.into());
    let conn = rustls::ClientConnection::new(Arc::new(cfg), name)?;
    let quick = QUITTING.load(std::sync::atomic::Ordering::Relaxed);
    let (connect, io) = if quick { (Duration::from_millis(600), Duration::from_millis(800)) } else { (Duration::from_secs(4), Duration::from_secs(6)) };
    let sock = TcpStream::connect_timeout(&(addr, 443).into(), connect)?;
    sock.set_read_timeout(Some(io))?;
    sock.set_write_timeout(Some(io))?;
    let mut tls = rustls::StreamOwned::new(conn, sock);
    let body = body.unwrap_or("");
    let mut req = format!("{method} {path} HTTP/1.1\r\nHost: {ip}\r\nConnection: close\r\nContent-Length: {}\r\n", body.len());
    if !key.is_empty() {
        req += &format!("hue-application-key: {key}\r\n");
    }
    if !body.is_empty() {
        req += "Content-Type: application/json\r\n";
    }
    req += "\r\n";
    req += body;
    tls.write_all(req.as_bytes())?;
    let mut raw = Vec::new();
    match tls.read_to_end(&mut raw) {
        Ok(_) => {}
        // The bridge closes without close_notify.
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof && !raw.is_empty() => {}
        Err(e) => return Err(e.into()),
    }
    let text = String::from_utf8_lossy(&raw).into_owned();
    let (head, rest) = text.split_once("\r\n\r\n").ok_or_else(|| anyhow!("bad reply from the bridge"))?;
    let status = head.split_whitespace().nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    let body = if head.to_ascii_lowercase().contains("transfer-encoding: chunked") { dechunk(rest) } else { rest.to_string() };
    let seen = verifier.seen.lock().unwrap_or_else(|e| e.into_inner()).map(|f| hex(&f)).unwrap_or_default();
    Ok(Reply { status, headers: head.to_string(), body, cert_sha256: seen })
}

fn dechunk(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some((len, tail)) = rest.split_once("\r\n") {
        let n = usize::from_str_radix(len.trim(), 16).unwrap_or(0);
        if n == 0 || tail.len() < n {
            break;
        }
        out.push_str(&tail[..n]);
        rest = tail[n..].trim_start_matches("\r\n");
    }
    out
}

fn json(r: &Reply) -> Result<serde_json::Value> {
    if r.status >= 400 {
        bail!("bridge said {}: {}", r.status, r.body.chars().take(200).collect::<String>());
    }
    serde_json::from_str(&r.body).context("bridge reply")
}

// ---- finding, pairing, areas ------------------------------------------------

/// Bridges on the network (`_hue._tcp`): (ip, name).
pub fn find_bridges() -> Vec<(String, String)> {
    let mut out = Vec::new();
    let Ok(d) = mdns_sd::ServiceDaemon::new() else { return out };
    if let Ok(rx) = d.browse("_hue._tcp.local.") {
        let end = std::time::Instant::now() + Duration::from_secs(4);
        while let Some(left) = end.checked_duration_since(std::time::Instant::now()) {
            match rx.recv_timeout(left) {
                Ok(mdns_sd::ServiceEvent::ServiceResolved(info)) => {
                    if let Some(a) = info.get_addresses_v4().iter().next() {
                        let ip = a.to_string();
                        let name = info.get_fullname().split("._hue").next().unwrap_or("Hue Bridge").to_string();
                        if !out.iter().any(|(i, _)| i == &ip) {
                            out.push((ip, name));
                        }
                    }
                }
                Ok(_) => {}
                Err(_) => break,
            }
        }
    }
    let _ = d.shutdown();
    out
}

/// Try to pair once: Ok(None) while the link button hasn't been pressed.
pub fn pair_once(ip: &str, name: &str) -> Result<Option<HueBridge>> {
    let r = call(ip, "", "POST", "/api", "", Some(r#"{"devicetype":"trippin#pc","generateclientkey":true}"#))?;
    let v = json(&r)?;
    let first = &v[0];
    if first["error"]["type"].as_i64() == Some(101) {
        return Ok(None);
    }
    let ok = &first["success"];
    let (Some(user), Some(ck)) = (ok["username"].as_str(), ok["clientkey"].as_str()) else {
        bail!("pairing failed: {}", r.body.chars().take(200).collect::<String>());
    };
    Ok(Some(HueBridge {
        ip: ip.into(),
        name: name.into(),
        user: user.into(),
        client_key: ck.into(),
        cert_sha256: r.cert_sha256,
        ..Default::default()
    }))
}

/// The bridge's entertainment areas: (id, name, channel count).
pub fn areas(b: &HueBridge) -> Result<Vec<(String, String, usize)>> {
    let r = call(&b.ip, &b.cert_sha256, "GET", "/clip/v2/resource/entertainment_configuration", &b.user, None)?;
    let v = json(&r)?;
    Ok(v["data"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|e| {
                    (
                        e["id"].as_str().unwrap_or("").to_string(),
                        e["metadata"]["name"].as_str().unwrap_or("area").to_string(),
                        e["channels"].as_array().map_or(0, |c| c.len()),
                    )
                })
                .collect()
        })
        .unwrap_or_default())
}

/// One channel of an area: id and its position across the room (-1..1).
#[derive(Clone, Copy, Debug)]
pub struct Channel {
    pub id: u8,
    pub x: f32,
}

fn area_channels(b: &HueBridge) -> Result<Vec<Channel>> {
    let r = call(&b.ip, &b.cert_sha256, "GET", &format!("/clip/v2/resource/entertainment_configuration/{}", b.area), &b.user, None)?;
    let v = json(&r)?;
    let chans = v["data"][0]["channels"].as_array().cloned().unwrap_or_default();
    Ok(chans
        .iter()
        .filter_map(|c| {
            Some(Channel {
                id: c["channel_id"].as_u64()? as u8,
                x: c["position"]["x"].as_f64().unwrap_or(0.0) as f32,
            })
        })
        .collect())
}

fn set_area(b: &HueBridge, action: &str) -> Result<()> {
    let r = call(
        &b.ip,
        &b.cert_sha256,
        "PUT",
        &format!("/clip/v2/resource/entertainment_configuration/{}", b.area),
        &b.user,
        Some(&format!(r#"{{"action":"{action}"}}"#)),
    )?;
    json(&r).map(|_| ())
}

/// Hand the lights back (quit, lights off). Best effort.
pub fn stop(b: &HueBridge) {
    if !b.area.is_empty() {
        let _ = set_area(b, "stop");
    }
}

// ---- the stream -------------------------------------------------------------

pub struct Stream {
    rt: tokio::runtime::Runtime,
    conn: webrtc_dtls::conn::DTLSConn,
    area: String,
    pub channels: Vec<Channel>,
    seq: u8,
}

impl Stream {
    /// Start the area and open the DTLS stream.
    pub fn open(b: &HueBridge) -> Result<Stream> {
        if b.area.is_empty() {
            bail!("no entertainment area chosen");
        }
        let auth = call(&b.ip, &b.cert_sha256, "GET", "/auth/v1", &b.user, None)?;
        let app_id = auth
            .headers
            .lines()
            .find_map(|l| l.split_once(':').filter(|(k, _)| k.trim().eq_ignore_ascii_case("hue-application-id")))
            .map(|(_, v)| v.trim().to_string())
            .ok_or_else(|| anyhow!("the bridge sent no application id"))?;
        let channels = area_channels(b)?;
        set_area(b, "start")?;
        let psk = unhex(&b.client_key).ok_or_else(|| anyhow!("bad client key"))?;
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
        let ip = b.ip.clone();
        let conn = rt.block_on(async move {
            let sock = tokio::net::UdpSocket::bind("0.0.0.0:0").await?;
            sock.connect(format!("{ip}:2100")).await?;
            let cfg = webrtc_dtls::config::Config {
                psk: Some(Arc::new(move |_hint: &[u8]| Ok(psk.clone()))),
                psk_identity_hint: Some(app_id.into_bytes()),
                cipher_suites: vec![webrtc_dtls::cipher_suite::CipherSuiteId::Tls_Psk_With_Aes_128_Gcm_Sha256],
                ..Default::default()
            };
            tokio::time::timeout(
                Duration::from_secs(8),
                webrtc_dtls::conn::DTLSConn::new(Arc::new(sock), cfg, true, None),
            )
            .await
            .map_err(|_| anyhow!("Hue stream handshake timed out"))?
            .map_err(|e| anyhow!("Hue stream handshake: {e}"))
        });
        let conn = match conn {
            Ok(c) => c,
            Err(e) => {
                let _ = set_area(b, "stop");
                return Err(e);
            }
        };
        Ok(Stream { rt, conn, area: b.area.clone(), channels, seq: 0 })
    }

    /// Send one frame: RGB 0..1 per channel, in `channels` order.
    pub fn send(&mut self, rgb: &[[f32; 3]]) -> Result<()> {
        let p = packet(&self.area, self.seq, &self.channels, rgb);
        self.seq = self.seq.wrapping_add(1);
        self.rt
            .block_on(self.conn.write(&p, Some(Duration::from_millis(200))))
            .map(|_| ())
            .map_err(|e| anyhow!("Hue stream: {e}"))
    }

    pub fn close(self, b: &HueBridge) {
        let _ = self.rt.block_on(self.conn.close());
        stop(b);
    }
}

/// A HueStream v2 packet (RGB colour space).
fn packet(area: &str, seq: u8, channels: &[Channel], rgb: &[[f32; 3]]) -> Vec<u8> {
    let mut p = Vec::with_capacity(52 + 7 * channels.len());
    p.extend_from_slice(b"HueStream");
    p.extend_from_slice(&[2, 0, seq, 0, 0, 0, 0]);
    p.extend_from_slice(area.as_bytes());
    for (c, col) in channels.iter().zip(rgb) {
        p.push(c.id);
        for v in col {
            p.extend_from_slice(&((v.clamp(0.0, 1.0) * 65535.0) as u16).to_be_bytes());
        }
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packet_layout() {
        let area = "811afa55-bef1-4d1f-800f-ecca6179461c";
        let ch = [Channel { id: 3, x: 0.0 }, Channel { id: 7, x: 0.5 }];
        let p = packet(area, 9, &ch, &[[1.0, 0.0, 0.5], [0.0, 1.0, 0.0]]);
        assert_eq!(&p[..9], b"HueStream");
        assert_eq!(&p[9..16], &[2, 0, 9, 0, 0, 0, 0]);
        assert_eq!(&p[16..52], area.as_bytes());
        assert_eq!(p.len(), 52 + 14);
        assert_eq!(&p[52..59], &[3, 0xff, 0xff, 0, 0, 0x7f, 0xff]);
        assert_eq!(p[59], 7);
    }

    #[test]
    fn chunked_replies_decode() {
        assert_eq!(dechunk("5\r\nhello\r\n6\r\n world\r\n0\r\n\r\n"), "hello world");
    }
}

// ---- background jobs for the Settings card ---------------------------------

/// What the Settings card's Hue section is waiting on / was handed back.
#[derive(Default)]
pub struct Ui {
    pub busy: bool,
    pub msg: String,
    pub found: Vec<(String, String)>,
    /// Paired: the panel moves it into Settings.
    pub paired: Option<HueBridge>,
    /// Areas of the paired bridge: (id, name, lights).
    pub areas: Vec<(String, String, usize)>,
}

pub static UI: Mutex<Ui> = Mutex::new(Ui { busy: false, msg: String::new(), found: Vec::new(), paired: None, areas: Vec::new() });

fn ui() -> std::sync::MutexGuard<'static, Ui> {
    UI.lock().unwrap_or_else(|e| e.into_inner())
}

fn job(name: &str, f: impl FnOnce() + Send + 'static) {
    {
        let mut u = ui();
        if u.busy {
            return;
        }
        u.busy = true;
    }
    let _ = std::thread::Builder::new().name(name.into()).spawn(move || {
        f();
        ui().busy = false;
    });
}

pub fn find_bg() {
    job("hue-find", || {
        ui().msg = "looking for bridges…".into();
        let f = find_bridges();
        let mut u = ui();
        u.msg = if f.is_empty() { "no Hue bridge found".into() } else { String::new() };
        u.found = f;
    });
}

/// Poll pairing for 40 s while the user presses the bridge's button.
pub fn pair_bg(ip: String, name: String) {
    job("hue-pair", move || {
        ui().msg = "press the link button on the bridge…".into();
        let end = std::time::Instant::now() + Duration::from_secs(40);
        while std::time::Instant::now() < end {
            match pair_once(&ip, &name) {
                Ok(Some(b)) => {
                    let areas = areas(&b).unwrap_or_default();
                    let mut u = ui();
                    u.msg = format!("paired with {name}");
                    u.areas = areas;
                    u.paired = Some(b);
                    return;
                }
                Ok(None) => std::thread::sleep(Duration::from_secs(2)),
                Err(e) => {
                    ui().msg = format!("{e:#}");
                    return;
                }
            }
        }
        ui().msg = "the button wasn't pressed in time".into();
    });
}

pub fn areas_bg(b: HueBridge) {
    job("hue-areas", move || match areas(&b) {
        Ok(a) => {
            let mut u = ui();
            u.msg = if a.is_empty() { "no entertainment areas: make one in the Hue app".into() } else { String::new() };
            u.areas = a;
        }
        Err(e) => ui().msg = format!("{e:#}"),
    });
}

/// `trippin --hue-test <ip> <user> <client key>`: pin the bridge, list its
/// areas, then stream 8 s of a synthetic groove to the first one.
pub fn cli_test(ip: &str, user: &str, client_key: &str) -> Result<()> {
    let first = call(ip, "", "GET", "/api/0/config", "", None)?;
    println!("bridge {} pinned {}…", json(&first)?["name"].as_str().unwrap_or("?"), &first.cert_sha256[..16]);
    let mut b = HueBridge {
        ip: ip.into(),
        user: user.into(),
        client_key: client_key.into(),
        cert_sha256: first.cert_sha256,
        ..Default::default()
    };
    // A second call must pass the pin.
    let a = areas(&b)?;
    for (id, name, n) in &a {
        println!("area {name} ({n} lights) {}", &id[..8]);
    }
    let (id, name, _) = a.first().cloned().ok_or_else(|| anyhow!("no entertainment areas"))?;
    b.area = id;
    b.area_name = name;
    let mut s = Stream::open(&b)?;
    println!("streaming to {} channels", s.channels.len());
    let lut = Arc::new(crate::palettes::lut("sunset"));
    let t0 = std::time::Instant::now();
    let mut rgb = Vec::new();
    while t0.elapsed() < Duration::from_secs(8) {
        let t = t0.elapsed().as_secs_f32();
        let pos = t as f64 * 126.0 / 60.0;
        let f = crate::lights::LightFrame {
            at: std::time::Instant::now(),
            lut: lut.clone(),
            hue: 0.0,
            pos,
            bpm: 126.0,
            beat_in_bar: (pos.floor() as u64) % 4,
            calm: 0.0,
            intensity: 0.7,
            energy: 0.7,
            hits4: [0.8, 0.4, 0.3, 0.2],
            pres4: [0.6, 0.5, 0.3, 0.2],
            master: 1.0,
            strobe_live: false,
            strobe_open: false,
            smooth: false,
        };
        crate::lights::render_hue(&mut rgb, &f, &s.channels, 0.0, 0.8, std::time::Instant::now());
        s.send(&rgb)?;
        std::thread::sleep(Duration::from_millis(40));
    }
    s.close(&b);
    println!("done; the area is stopped and the lights handed back");
    Ok(())
}

// ---- lights outside the entertainment stream --------------------------------
//
// Non-Hue bulbs on the bridge (Gledopto, Tuya, innr…) can't join an
// entertainment area. They're driven with plain light commands instead, which
// the bridge takes only ~10 a second in total — so they follow the show at
// the bar, not the beat (`lights.rs::slow_worker`).

/// A light on the bridge: id, name, room-free product name, colour, can stream.
#[derive(Clone, Debug)]
pub struct BridgeLight {
    pub id: String,
    pub name: String,
    pub maker: String,
    pub colour: bool,
    pub streams: bool,
}

pub fn lights(b: &HueBridge) -> Result<Vec<BridgeLight>> {
    let get = |p: &str| -> Result<Vec<serde_json::Value>> {
        let r = call(&b.ip, &b.cert_sha256, "GET", p, &b.user, None)?;
        Ok(json(&r)?["data"].as_array().cloned().unwrap_or_default())
    };
    let devices = get("/clip/v2/resource/device")?;
    let ents = get("/clip/v2/resource/entertainment")?;
    let mut out: Vec<BridgeLight> = get("/clip/v2/resource/light")?
        .iter()
        .map(|l| {
            let owner = l["owner"]["rid"].as_str().unwrap_or("");
            let dev = devices.iter().find(|d| d["id"].as_str() == Some(owner));
            BridgeLight {
                id: l["id"].as_str().unwrap_or("").into(),
                name: l["metadata"]["name"].as_str().unwrap_or("light").into(),
                maker: dev.and_then(|d| d["product_data"]["manufacturer_name"].as_str()).unwrap_or("").into(),
                colour: l.get("color").is_some(),
                streams: ents
                    .iter()
                    .any(|e| e["owner"]["rid"].as_str() == Some(owner) && e["renderer"].as_bool() == Some(true)),
            }
        })
        .collect();
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(out)
}

/// sRGB 0..1 to CIE xy (Hue's colour space), with its brightness 0..100.
pub fn rgb_to_xy(rgb: [f32; 3]) -> ([f32; 2], f32) {
    let lin = |c: f32| if c > 0.04045 { ((c + 0.055) / 1.055).powf(2.4) } else { c / 12.92 };
    let [r, g, b] = rgb.map(|c| lin(c.clamp(0.0, 1.0)));
    let x = r * 0.4124 + g * 0.3576 + b * 0.1805;
    let y = r * 0.2126 + g * 0.7152 + b * 0.0722;
    let z = r * 0.0193 + g * 0.1192 + b * 0.9505;
    let sum = x + y + z;
    let xy = if sum > 1e-6 { [x / sum, y / sum] } else { [0.3127, 0.3290] };
    let bri = rgb.iter().cloned().fold(0.0f32, f32::max) * 100.0;
    (xy, bri)
}

/// Set one light: colour, brightness (0 = off) and a fade time.
pub fn set_light(b: &HueBridge, id: &str, rgb: [f32; 3], fade_ms: u32) -> Result<()> {
    let (xy, bri) = rgb_to_xy(rgb);
    let body = if bri < 1.0 {
        format!(r#"{{"on":{{"on":false}},"dynamics":{{"duration":{fade_ms}}}}}"#)
    } else {
        format!(
            r#"{{"on":{{"on":true}},"dimming":{{"brightness":{bri:.1}}},"color":{{"xy":{{"x":{:.4},"y":{:.4}}}}},"dynamics":{{"duration":{fade_ms}}}}}"#,
            xy[0], xy[1]
        )
    };
    let r = call(&b.ip, &b.cert_sha256, "PUT", &format!("/clip/v2/resource/light/{id}"), &b.user, Some(&body))?;
    json(&r).map(|_| ())
}

pub fn lights_bg(b: HueBridge) {
    job("hue-lights", move || match lights(&b) {
        Ok(l) => *LIGHTS.lock().unwrap_or_else(|e| e.into_inner()) = l,
        Err(e) => ui().msg = format!("{e:#}"),
    });
}

/// The bridge's lights, for the "slow follow" picker.
pub static LIGHTS: Mutex<Vec<BridgeLight>> = Mutex::new(Vec::new());

/// A light's state before the show took it, to put back afterwards.
#[derive(Clone, Debug)]
pub struct Saved {
    pub id: String,
    pub body: String,
}

pub fn save_light(b: &HueBridge, id: &str) -> Option<Saved> {
    let r = call(&b.ip, &b.cert_sha256, "GET", &format!("/clip/v2/resource/light/{id}"), &b.user, None).ok()?;
    let v = json(&r).ok()?;
    let l = &v["data"][0];
    let mut body = serde_json::json!({ "on": { "on": l["on"]["on"].as_bool().unwrap_or(true) } });
    if let Some(bri) = l["dimming"]["brightness"].as_f64() {
        body["dimming"] = serde_json::json!({ "brightness": bri });
    }
    if l["color_temperature"]["mirek_valid"].as_bool() == Some(true) {
        body["color_temperature"] = serde_json::json!({ "mirek": l["color_temperature"]["mirek"] });
    } else if let Some(xy) = l["color"]["xy"].as_object() {
        body["color"] = serde_json::json!({ "xy": xy });
    }
    Some(Saved { id: id.into(), body: body.to_string() })
}

pub fn restore_light(b: &HueBridge, s: &Saved) {
    let _ = call(&b.ip, &b.cert_sha256, "PUT", &format!("/clip/v2/resource/light/{}", s.id), &b.user, Some(&s.body));
}

/// `trippin --hue-slow-test <ip> <user> <client key> <name,name,…> [s]`:
/// drive the named non-streaming lights through the real worker with a
/// synthetic groove, then stop (each light's own state put back).
pub fn cli_slow_test(ip: &str, user: &str, client_key: &str, names: &str, secs: f32) -> Result<()> {
    let first = call(ip, "", "GET", "/api/0/config", "", None)?;
    let b = HueBridge { ip: ip.into(), user: user.into(), client_key: client_key.into(), cert_sha256: first.cert_sha256, ..Default::default() };
    let all = lights(&b)?;
    let want: Vec<&str> = names.split(',').map(str::trim).collect();
    let ids: Vec<String> = all.iter().filter(|l| want.contains(&l.name.as_str())).map(|l| l.id.clone()).collect();
    println!("{} of {} named lights found", ids.len(), want.len());
    let lights = crate::lights::Lights::start();
    let mut s = crate::config::Settings::default();
    s.lights_on = true;
    s.hue = b;
    s.hue_slow = ids;
    let lut = Arc::new(crate::palettes::lut("sunset"));
    let t0 = std::time::Instant::now();
    while t0.elapsed().as_secs_f32() < secs {
        let pos = t0.elapsed().as_secs_f64() * 126.0 / 60.0;
        lights.update(
            crate::lights::LightFrame {
                at: std::time::Instant::now(),
                lut: lut.clone(),
                hue: 0.0,
                pos,
                bpm: 126.0,
                beat_in_bar: (pos.floor() as u64) % 4,
                calm: 0.0,
                intensity: 0.8,
                energy: 0.7,
                hits4: [0.8, 0.4, 0.3, 0.2],
                pres4: [0.6, 0.5, 0.3, 0.2],
                master: 1.0,
                strobe_live: false,
                strobe_open: false,
                smooth: false,
            },
            &s,
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    s.lights_on = false;
    lights.update(
        crate::lights::LightFrame {
            at: std::time::Instant::now(),
            lut,
            hue: 0.0,
            pos: 0.0,
            bpm: 126.0,
            beat_in_bar: 0,
            calm: 0.0,
            intensity: 0.0,
            energy: 0.0,
            hits4: [0.0; 4],
            pres4: [0.0; 4],
            master: 1.0,
            strobe_live: false,
            strobe_open: false,
            smooth: false,
        },
        &s,
    );
    std::thread::sleep(Duration::from_secs(4));
    println!("done; lights put back");
    Ok(())
}
