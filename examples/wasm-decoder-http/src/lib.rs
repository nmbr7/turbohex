/// turbohex WASM Decoder — HTTP Message Parser
///
/// Uses `httparse` (the same parser hyper uses) to parse HTTP/1.x messages
/// with range-mapped fields for hex view highlighting.
///
/// ABI contract:
///   - Export `alloc(size: i32) -> i32`
///   - Export `decode(ptr: i32, len: i32, endian: i32) -> i32`
///       returns NUL-terminated JSON:
///       [{"label":"...","value":"...","offset":N,"length":N},...]
///
/// Build:
///   rustup target add wasm32-unknown-unknown
///   cargo build --target wasm32-unknown-unknown --release
///   cp target/wasm32-unknown-unknown/release/decoder_http.wasm \
///      ~/.config/turbohex/decoders/

use std::alloc::Layout;
use std::slice;

#[unsafe(no_mangle)]
pub extern "C" fn alloc(size: i32) -> i32 {
    let layout = Layout::from_size_align(size as usize, 1).unwrap();
    let ptr = unsafe { std::alloc::alloc(layout) };
    ptr as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn decode(ptr: i32, len: i32, _endian: i32) -> i32 {
    let bytes = unsafe { slice::from_raw_parts(ptr as *const u8, len as usize) };

    let mut out = JsonBuilder::new();

    if bytes.len() < 10 {
        return out.finish();
    }

    // Try parsing as request first, then response
    if try_parse_request(&mut out, bytes) || try_parse_response(&mut out, bytes) {
        // parsed
    }

    out.finish()
}

fn try_parse_request(out: &mut JsonBuilder, bytes: &[u8]) -> bool {
    let mut headers = [httparse::EMPTY_HEADER; 64];
    let mut req = httparse::Request::new(&mut headers);

    let status = match req.parse(bytes) {
        Ok(s) => s,
        Err(_) => return false,
    };

    let method = match req.method {
        Some(m) => m,
        None => return false,
    };

    // Method range: starts at 0
    let method_len = method.len();
    out.entry_range("Method", method, 0, method_len);

    // URI: after "METHOD "
    if let Some(path) = req.path {
        let uri_offset = method_len + 1; // space after method
        let uri_len = path.len();
        out.entry_range("URI", path, uri_offset, uri_len);

        // Parse query string
        if let Some(q) = path.find('?') {
            out.entry("  Query", &path[q + 1..]);
        }
    }

    // Version: "HTTP/1.x" at end of request line
    if let Some(version) = req.version {
        let ver_str = match version {
            0 => "HTTP/1.0",
            1 => "HTTP/1.1",
            _ => "HTTP/1.?",
        };
        // version sits right before the \r\n of the first line
        // "METHOD URI HTTP/1.x\r\n" — version is 8 bytes
        if let Some(crlf) = find_crlf(bytes, 0) {
            let ver_offset = crlf - 8;
            out.entry_range("Version", ver_str, ver_offset, 8);
        }
    }

    out.entry("", ""); // blank separator

    // Headers with range mapping
    let header_start = find_crlf(bytes, 0).map(|p| p + 2).unwrap_or(0);
    emit_headers(out, bytes, &req.headers, header_start);

    // Body
    if let httparse::Status::Complete(head_len) = status {
        if head_len < bytes.len() {
            let body = &bytes[head_len..];
            let body_len = body.len();
            let preview = safe_ascii_preview(body, 60);
            out.entry("", ""); // blank separator
            out.entry_range("Body", &preview, head_len, body_len);
            out.entry("Body Length", &fmt_usize(body_len));
        }
    }

    true
}

fn try_parse_response(out: &mut JsonBuilder, bytes: &[u8]) -> bool {
    let mut headers = [httparse::EMPTY_HEADER; 64];
    let mut resp = httparse::Response::new(&mut headers);

    let status = match resp.parse(bytes) {
        Ok(s) => s,
        Err(_) => return false,
    };

    // Version
    if let Some(version) = resp.version {
        let ver_str = match version {
            0 => "HTTP/1.0",
            1 => "HTTP/1.1",
            _ => "HTTP/1.?",
        };
        out.entry_range("Version", ver_str, 0, 8);
    }

    // Status code: "HTTP/1.x NNN"
    if let Some(code) = resp.code {
        let code_str = fmt_u16(code);
        out.entry_range("Status Code", &code_str, 9, 3);

        let desc = match code {
            100 => "Continue",
            200 => "OK",
            201 => "Created",
            204 => "No Content",
            301 => "Moved Permanently",
            302 => "Found",
            304 => "Not Modified",
            400 => "Bad Request",
            401 => "Unauthorized",
            403 => "Forbidden",
            404 => "Not Found",
            405 => "Method Not Allowed",
            500 => "Internal Server Error",
            502 => "Bad Gateway",
            503 => "Service Unavailable",
            _ => "",
        };
        if !desc.is_empty() {
            out.entry("  Status", desc);
        }
    }

    // Reason phrase
    if let Some(reason) = resp.reason {
        // "HTTP/1.x NNN REASON\r\n" — reason starts at offset 13
        let reason_offset = 13;
        out.entry_range("Reason", reason, reason_offset, reason.len());
    }

    out.entry("", ""); // blank separator

    // Headers
    let header_start = find_crlf(bytes, 0).map(|p| p + 2).unwrap_or(0);
    emit_headers(out, bytes, &resp.headers, header_start);

    // Body
    if let httparse::Status::Complete(head_len) = status {
        if head_len < bytes.len() {
            let body = &bytes[head_len..];
            let body_len = body.len();
            let preview = safe_ascii_preview(body, 60);
            out.entry("", ""); // blank separator
            out.entry_range("Body", &preview, head_len, body_len);
            out.entry("Body Length", &fmt_usize(body_len));
        }
    }

    true
}

fn emit_headers(out: &mut JsonBuilder, bytes: &[u8], headers: &[httparse::Header], start: usize) {
    let mut pos = start;
    let mut count = 0u32;

    for hdr in headers {
        if hdr.name.is_empty() {
            break;
        }

        // Each header line: "Name: Value\r\n"
        let hdr_end = find_crlf(bytes, pos).unwrap_or(bytes.len());
        let hdr_len = hdr_end - pos;

        let value = core::str::from_utf8(hdr.value).unwrap_or("<binary>");
        out.entry_range(hdr.name, value, pos, hdr_len);

        // Extra info for well-known headers
        let name_lower: String = hdr.name.chars().map(|c| {
            if c >= 'A' && c <= 'Z' { (c as u8 + 32) as char } else { c }
        }).collect();

        if name_lower == "content-type" {
            if let Some(semi) = value.find(';') {
                out.entry("  MIME", value[..semi].trim());
                out.entry("  Params", value[semi + 1..].trim());
            }
        }

        pos = if hdr_end + 2 <= bytes.len() { hdr_end + 2 } else { bytes.len() };
        count += 1;
    }

    out.entry("", ""); // blank separator
    out.entry("Header Count", &fmt_u32(count));
}

// --- Helpers ---

fn find_crlf(bytes: &[u8], start: usize) -> Option<usize> {
    let mut i = start;
    while i + 1 < bytes.len() {
        if bytes[i] == b'\r' && bytes[i + 1] == b'\n' {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn safe_ascii_preview(bytes: &[u8], max: usize) -> String {
    let limit = bytes.len().min(max);
    let mut s = String::with_capacity(limit + 3);
    for &b in &bytes[..limit] {
        if b >= 0x20 && b <= 0x7e {
            s.push(b as char);
        } else {
            s.push('.');
        }
    }
    if bytes.len() > max {
        s.push_str("...");
    }
    s
}

fn fmt_usize(v: usize) -> String {
    if v == 0 { return "0".into(); }
    let mut buf = [0u8; 20];
    let mut i = 0;
    let mut n = v;
    while n > 0 { buf[i] = b'0' + (n % 10) as u8; n /= 10; i += 1; }
    let mut s = String::with_capacity(i);
    while i > 0 { i -= 1; s.push(buf[i] as char); }
    s
}

fn fmt_u16(v: u16) -> String { fmt_usize(v as usize) }
fn fmt_u32(v: u32) -> String { fmt_usize(v as usize) }

// --- JSON builder ---

struct JsonBuilder {
    json: String,
    first: bool,
}

impl JsonBuilder {
    fn new() -> Self {
        Self { json: String::from("["), first: true }
    }

    fn entry(&mut self, label: &str, value: &str) {
        if !self.first { self.json.push(','); }
        self.first = false;
        self.json.push_str(r#"{"label":""#);
        json_escape(&mut self.json, label);
        self.json.push_str(r#"","value":""#);
        json_escape(&mut self.json, value);
        self.json.push_str(r#""}"#);
    }

    fn entry_range(&mut self, label: &str, value: &str, offset: usize, length: usize) {
        if !self.first { self.json.push(','); }
        self.first = false;
        self.json.push_str(r#"{"label":""#);
        json_escape(&mut self.json, label);
        self.json.push_str(r#"","value":""#);
        json_escape(&mut self.json, value);
        self.json.push_str(r#"","offset":"#);
        self.json.push_str(&fmt_usize(offset));
        self.json.push_str(r#","length":"#);
        self.json.push_str(&fmt_usize(length));
        self.json.push('}');
    }

    fn finish(mut self) -> i32 {
        self.json.push(']');
        self.json.push('\0');
        let bytes = self.json.into_bytes();
        let ptr = bytes.as_ptr() as i32;
        std::mem::forget(bytes);
        ptr
    }
}

fn json_escape(out: &mut String, s: &str) {
    for c in s.chars() {
        match c {
            '"' => out.push_str(r#"\""#),
            '\\' => out.push_str(r#"\\"#),
            '\n' => out.push_str(r#"\n"#),
            '\t' => out.push_str(r#"\t"#),
            _ => out.push(c),
        }
    }
}
