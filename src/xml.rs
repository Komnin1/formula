//! A small XML reader/writer, enough for CDXML and CML files.

#[derive(Clone, Debug, Default)]
pub struct Node {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Child>,
}

#[derive(Clone, Debug)]
pub enum Child {
    El(Node),
    Text(String),
}

impl Node {
    pub fn attr(&self, k: &str) -> Option<&str> {
        self.attrs.iter().find(|(a, _)| a == k).map(|(_, v)| v.as_str())
    }
    pub fn elements(&self) -> impl Iterator<Item = &Node> {
        self.children.iter().filter_map(|c| if let Child::El(n) = c { Some(n) } else { None })
    }
    pub fn text(&self) -> String {
        let mut s = String::new();
        for c in &self.children {
            match c {
                Child::Text(t) => s.push_str(t),
                Child::El(n) => s.push_str(&n.text()),
            }
        }
        s
    }
    /// Local name without a namespace prefix.
    pub fn local(&self) -> &str {
        self.name.rsplit(':').next().unwrap_or(&self.name)
    }
}

pub fn unescape(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        if let Some(j) = rest.find(';') {
            let ent = &rest[1..j];
            let rep = match ent {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                _ if ent.starts_with("#x") => u32::from_str_radix(&ent[2..], 16).ok().and_then(char::from_u32),
                _ if ent.starts_with('#') => ent[1..].parse().ok().and_then(char::from_u32),
                _ => None,
            };
            if let Some(c) = rep {
                out.push(c);
                rest = &rest[j + 1..];
                continue;
            }
        }
        out.push('&');
        rest = &rest[1..];
    }
    out.push_str(rest);
    out
}

pub fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

pub fn parse(src: &str) -> Result<Node, String> {
    let b = src.as_bytes();
    let mut i = 0;
    let mut stack: Vec<Node> = vec![Node { name: "#root".into(), ..Default::default() }];
    while i < b.len() {
        if b[i] == b'<' {
            if src[i..].starts_with("<!--") {
                i = src[i..].find("-->").map(|j| i + j + 3).ok_or("unclosed comment")?;
                continue;
            }
            if src[i..].starts_with("<![CDATA[") {
                let j = src[i..].find("]]>").ok_or("unclosed CDATA")?;
                let t = &src[i + 9..i + j];
                stack.last_mut().unwrap().children.push(Child::Text(t.to_string()));
                i += j + 3;
                continue;
            }
            if src[i..].starts_with("<?") || src[i..].starts_with("<!") {
                // Declarations and DOCTYPE (may contain brackets).
                let mut depth = 0;
                let mut j = i + 1;
                while j < b.len() {
                    match b[j] {
                        b'[' => depth += 1,
                        b']' => depth -= 1,
                        b'>' if depth <= 0 => break,
                        _ => {}
                    }
                    j += 1;
                }
                i = j + 1;
                continue;
            }
            if src[i..].starts_with("</") {
                let j = src[i..].find('>').ok_or("bad close tag")? + i;
                let n = stack.pop().ok_or("unbalanced")?;
                if stack.is_empty() {
                    return Err("unbalanced".into());
                }
                stack.last_mut().unwrap().children.push(Child::El(n));
                i = j + 1;
                continue;
            }
            // Start tag.
            let mut j = i + 1;
            while j < b.len() && !b[j].is_ascii_whitespace() && b[j] != b'>' && b[j] != b'/' {
                j += 1;
            }
            let name = src[i + 1..j].to_string();
            let mut node = Node { name, ..Default::default() };
            let mut self_close = false;
            loop {
                while j < b.len() && b[j].is_ascii_whitespace() {
                    j += 1;
                }
                if j >= b.len() {
                    return Err("unexpected end".into());
                }
                if b[j] == b'>' {
                    j += 1;
                    break;
                }
                if b[j] == b'/' {
                    self_close = true;
                    j += 1;
                    continue;
                }
                let ks = j;
                while j < b.len() && b[j] != b'=' && !b[j].is_ascii_whitespace() && b[j] != b'>' {
                    j += 1;
                }
                let key = src[ks..j].to_string();
                while j < b.len() && b[j].is_ascii_whitespace() {
                    j += 1;
                }
                if j < b.len() && b[j] == b'=' {
                    j += 1;
                    while j < b.len() && b[j].is_ascii_whitespace() {
                        j += 1;
                    }
                    let q = b[j];
                    if q == b'"' || q == b'\'' {
                        let vs = j + 1;
                        let ve = src[vs..].find(q as char).ok_or("unclosed attribute")? + vs;
                        node.attrs.push((key, unescape(&src[vs..ve])));
                        j = ve + 1;
                    } else {
                        let vs = j;
                        while j < b.len() && !b[j].is_ascii_whitespace() && b[j] != b'>' {
                            j += 1;
                        }
                        node.attrs.push((key, unescape(&src[vs..j])));
                    }
                } else {
                    node.attrs.push((key, String::new()));
                }
            }
            i = j;
            if self_close {
                stack.last_mut().unwrap().children.push(Child::El(node));
            } else {
                stack.push(node);
            }
        } else {
            let j = src[i..].find('<').map(|j| i + j).unwrap_or(b.len());
            let t = &src[i..j];
            if !t.trim().is_empty() {
                stack.last_mut().unwrap().children.push(Child::Text(unescape(t)));
            }
            i = j;
        }
    }
    while stack.len() > 1 {
        let n = stack.pop().unwrap();
        stack.last_mut().unwrap().children.push(Child::El(n));
    }
    let root = stack.pop().unwrap();
    root.children
        .into_iter()
        .find_map(|c| if let Child::El(n) = c { Some(n) } else { None })
        .ok_or_else(|| "no XML element found".to_string())
}

pub fn write(n: &Node, indent: usize, out: &mut String) {
    let pad = " ".repeat(indent);
    out.push_str(&pad);
    out.push('<');
    out.push_str(&n.name);
    for (k, v) in &n.attrs {
        out.push_str(&format!("\n{} {}=\"{}\"", pad, k, escape(v)));
    }
    if n.children.is_empty() {
        out.push_str("\n/>");
        out.push('\n');
        return;
    }
    out.push('>');
    let only_text = n.children.iter().all(|c| matches!(c, Child::Text(_)));
    if only_text {
        for c in &n.children {
            if let Child::Text(t) = c {
                out.push_str(&escape(t));
            }
        }
    } else {
        out.push('\n');
        for c in &n.children {
            match c {
                Child::El(e) => write(e, indent, out),
                Child::Text(t) => out.push_str(&escape(t)),
            }
        }
        out.push_str(&pad);
    }
    out.push_str(&format!("</{}>\n", n.name));
}

pub fn to_string(n: &Node) -> String {
    let mut s = String::new();
    write(n, 0, &mut s);
    s
}

pub fn el(name: &str, attrs: Vec<(&str, String)>) -> Node {
    Node { name: name.into(), attrs: attrs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), children: vec![] }
}
