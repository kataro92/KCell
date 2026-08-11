//! Tiny WASI Cell: one JSON envelope line in → one line out.
//! Build: `rustc --target wasm32-wasip1 -O -o echo.wasm main.rs`

fn main() {
    use std::io::{Read, Write};
    let mut buf = Vec::new();
    std::io::stdin().read_to_end(&mut buf).unwrap();
    let s = String::from_utf8_lossy(&buf);
    let id = json_str(&s, "correlationId").unwrap_or("\"missing\"");
    let cap = json_str(&s, "capability").unwrap_or("\"echo-wasi\"");
    let out = format!(
        "{{\"schema\":\"kcell.envelope.v1\",\"correlationId\":{id},\"capability\":{cap},\"payload\":{{\"runtime\":\"wasi\",\"ok\":true}}}}\n"
    );
    std::io::stdout().write_all(out.as_bytes()).unwrap();
}

fn json_str<'a>(s: &'a str, key: &str) -> Option<&'a str> {
    let pat = format!("\"{key}\"");
    let i = s.find(&pat)?;
    let rest = &s[i + pat.len()..];
    let colon = rest.find(':')?;
    let r = rest[colon + 1..].trim_start();
    if r.starts_with('"') {
        let end = r[1..].find('"')? + 1;
        Some(&r[..=end])
    } else {
        let end = r.find([',', '}']).unwrap_or(r.len());
        Some(r[..end].trim())
    }
}
