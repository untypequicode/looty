//! HTML <-> Yjs : ancien HTML des pages jamais ouvertes dans l'éditeur (converti seulement s'il ne
//! contient que des blocs que l'éditeur sait écrire sans perte) et copie HTML `pages.content`
//! régénérée après une écriture d'agent.

use super::*;

/// Marqueur d'un bloc ou d'une marque que la copie HTML ne sait pas écrire : `state_to_html` échoue alors.
const UNSUPPORTED: &str = "\u{0}";

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn html_text<T: ReadTxn>(txn: &T, t: &yrs::XmlTextRef) -> String {
    let mut out = String::new();
    for d in t.diff(txn, |_| ()) as Vec<Diff<()>> {
        let text = d.insert.to_string(txn);
        if text.is_empty() {
            continue;
        }
        let mut s = esc(&text);
        if let Some(attrs) = d.attributes.as_ref() {
            if attrs.keys().any(|k| !matches!(k.split("--").next(), Some("bold" | "italic" | "code" | "strike" | "link"))) {
                out.push_str(UNSUPPORTED);
            }
            let has = |n: &str| attrs.keys().any(|k| k.split("--").next() == Some(n));
            if has("code") {
                s = format!("<code>{s}</code>");
            }
            if has("bold") {
                s = format!("<strong>{s}</strong>");
            }
            if has("italic") {
                s = format!("<em>{s}</em>");
            }
            if has("strike") {
                s = format!("<s>{s}</s>");
            }
            if let Some((_, Any::Map(m))) = attrs.iter().find(|(k, _)| k.starts_with("link")) {
                if let Some(Any::String(h)) = m.get("href") {
                    s = format!("<a href=\"{}\">{s}</a>", esc(h));
                }
            }
        }
        out.push_str(&s);
    }
    out
}

fn html_inline<T: ReadTxn>(txn: &T, node: &impl XmlFragment) -> String {
    let mut out = String::new();
    for child in node.children(txn) {
        match child {
            XmlOut::Text(t) => out.push_str(&html_text(txn, &t)),
            XmlOut::Element(e) => match e.tag().as_ref() {
                "mathInline" => out.push_str(&format!("<span data-type=\"math-inline\" latex=\"{}\"></span>", esc(&plain(txn, &e)))),
                "hardBreak" => out.push_str("<br>"),
                _ => out.push_str(UNSUPPORTED),
            },
            XmlOut::Fragment(_) => {}
        }
    }
    out
}

fn html_children<T: ReadTxn>(txn: &T, node: &impl XmlFragment) -> String {
    node.children(txn).filter_map(|c| if let XmlOut::Element(e) = c { Some(html_block(txn, &e)) } else { None }).collect()
}

fn html_block<T: ReadTxn>(txn: &T, e: &yrs::XmlElementRef) -> String {
    match e.tag().as_ref() {
        "paragraph" => format!("<p>{}</p>", html_inline(txn, e)),
        "heading" => {
            let n: usize = attr_str(txn, e, "level").and_then(|l| l.parse::<f64>().ok()).map(|f| f as usize).unwrap_or(1).clamp(1, 6);
            format!("<h{n}>{}</h{n}>", html_inline(txn, e))
        }
        "bulletList" | "orderedList" => {
            let tag = if e.tag().as_ref() == "bulletList" { "ul" } else { "ol" };
            let items: String = e.children(txn).filter_map(|c| if let XmlOut::Element(li) = c { Some(format!("<li>{}</li>", html_children(txn, &li))) } else { None }).collect();
            format!("<{tag}>{items}</{tag}>")
        }
        "taskList" => {
            let items: String = e
                .children(txn)
                .filter_map(|c| {
                    let XmlOut::Element(li) = c else { return None };
                    let done = attr_str(txn, &li, "checked").as_deref() == Some("true");
                    Some(format!(
                        "<li data-checked=\"{done}\" data-type=\"taskItem\"><label><input type=\"checkbox\"{}><span></span></label><div>{}</div></li>",
                        if done { " checked=\"checked\"" } else { "" },
                        html_children(txn, &li)
                    ))
                })
                .collect();
            format!("<ul data-type=\"taskList\">{items}</ul>")
        }
        "blockquote" => format!("<blockquote>{}</blockquote>", html_children(txn, e)),
        "notice" => {
            let kind = esc(&attr_str(txn, e, "type").unwrap_or_else(|| "info".into()));
            format!("<blockquote data-notice-type=\"{kind}\" data-type=\"notice\" class=\"callout callout-{kind}\">{}</blockquote>", html_children(txn, e))
        }
        "codeBlock" => {
            let class = attr_str(txn, e, "language").filter(|l| !l.is_empty()).map(|l| format!(" class=\"language-{}\"", esc(&l))).unwrap_or_default();
            format!("<pre><code{class}>{}</code></pre>", esc(&plain(txn, e)))
        }
        "horizontalRule" => "<hr>".to_string(),
        "mathBlock" => format!("<div data-type=\"math-block\" latex=\"{}\"></div>", esc(&plain(txn, e))),
        "mermaid" => format!("<div data-type=\"mermaid\"><pre>{}</pre></div>", esc(&plain(txn, e))),
        "graph" => format!("<div data-type=\"graph\"><pre>{}</pre></div>", esc(&plain(txn, e))),
        "table" => {
            let rows: String = e
                .children(txn)
                .filter_map(|tr| {
                    let XmlOut::Element(tr) = tr else { return None };
                    let cells: String = tr
                        .children(txn)
                        .filter_map(|c| {
                            let XmlOut::Element(c) = c else { return None };
                            let tag = if c.tag().as_ref() == "tableHeader" { "th" } else { "td" };
                            Some(format!("<{tag}>{}</{tag}>", html_children(txn, &c)))
                        })
                        .collect();
                    Some(format!("<tr>{cells}</tr>"))
                })
                .collect();
            format!("<table><tbody>{rows}</tbody></table>")
        }
        "image" => match attr_str(txn, e, "src") {
            Some(src) => format!("<img src=\"{}\" alt=\"{}\">", esc(&src), esc(&attr_str(txn, e, "alt").unwrap_or_default())),
            None => String::new(),
        },
        // Blocs que ce module ne sait pas écrire (tableaux de bord, intégrations…) : absents de la copie HTML.
        _ => UNSUPPORTED.to_string(),
    }
}

/// HTML d'un état Yjs de page : la copie lisible `pages.content` (page publique, export, recherche),
/// dans le même format que celui produit par l'éditeur pour les blocs que ce module connaît.
pub fn state_to_html(state: &[u8]) -> Result<String, String> {
    let doc = load(state)?;
    let frag = doc.get_or_insert_xml_fragment(FRAGMENT);
    let txn = doc.transact();
    let html = html_children(&txn, &frag);
    if html.contains(UNSUPPORTED) {
        return Err("the document has blocks or marks the HTML copy cannot represent".into());
    }
    Ok(html)
}

#[derive(Debug)]
enum Tok {
    Open(String, Vec<(String, String)>),
    Close(String),
    Text(String),
}

fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let Some(end) = rest.find(';').filter(|e| *e <= 10) else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let ent = &rest[1..end];
        let rep = match ent {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some(' '),
            _ => ent
                .strip_prefix('#')
                .and_then(|n| n.strip_prefix(['x', 'X']).map_or_else(|| n.parse::<u32>().ok(), |h| u32::from_str_radix(h, 16).ok()))
                .and_then(char::from_u32),
        };
        match rep {
            Some(c) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Position du `>` qui ferme la balise commençant `rest` (hors valeurs d'attributs entre guillemets).
fn tag_end(rest: &str) -> Option<usize> {
    let mut quote: Option<char> = None;
    for (i, c) in rest.char_indices() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (None, '"' | '\'') => quote = Some(c),
            (None, '>') => return Some(i),
            _ => {}
        }
    }
    None
}

fn tokenize(html: &str) -> Vec<Tok> {
    let mut toks = vec![];
    let mut rest = html;
    while !rest.is_empty() {
        let Some(i) = rest.find('<') else {
            toks.push(Tok::Text(decode_entities(rest)));
            break;
        };
        if i > 0 {
            toks.push(Tok::Text(decode_entities(&rest[..i])));
        }
        rest = &rest[i..];
        if let Some(r) = rest.strip_prefix("<!--") {
            rest = r.find("-->").map_or("", |e| &r[e + 3..]);
            continue;
        }
        let Some(end) = tag_end(rest) else {
            toks.push(Tok::Text(decode_entities(rest)));
            break;
        };
        let inner = &rest[1..end];
        rest = &rest[end + 1..];
        if let Some(name) = inner.strip_prefix('/') {
            toks.push(Tok::Close(name.trim().to_lowercase()));
            continue;
        }
        let inner = inner.trim_end_matches('/');
        let name_end = inner.find(|c: char| c.is_whitespace()).unwrap_or(inner.len());
        let name = inner[..name_end].to_lowercase();
        let mut attrs = vec![];
        let mut a = inner[name_end..].trim_start();
        while !a.is_empty() {
            let k_end = a.find(|c: char| c == '=' || c.is_whitespace()).unwrap_or(a.len());
            let key = a[..k_end].to_lowercase();
            a = a[k_end..].trim_start();
            let mut val = String::new();
            if let Some(r) = a.strip_prefix('=') {
                let r = r.trim_start();
                if let Some(q) = r.chars().next().filter(|c| *c == '"' || *c == '\'') {
                    let body = &r[1..];
                    let e = body.find(q).unwrap_or(body.len());
                    val = decode_entities(&body[..e]);
                    a = body.get(e + 1..).unwrap_or("").trim_start();
                } else {
                    let e = r.find(char::is_whitespace).unwrap_or(r.len());
                    val = decode_entities(&r[..e]);
                    a = r[e..].trim_start();
                }
            }
            if !key.is_empty() {
                attrs.push((key, val));
            }
        }
        let void = matches!(name.as_str(), "br" | "hr" | "img" | "input" | "col" | "meta" | "link");
        toks.push(Tok::Open(name.clone(), attrs));
        if void {
            toks.push(Tok::Close(name));
        }
    }
    toks
}

struct HtmlParser {
    toks: Vec<Tok>,
    pos: usize,
}

fn attr<'a>(attrs: &'a [(String, String)], name: &str) -> Option<&'a str> {
    attrs.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
}

impl HtmlParser {
    /// Contenu en ligne jusqu'à une fermeture non ouverte ici ou un bloc (non consommé).
    fn inlines(&mut self) -> Result<Vec<Inline>, String> {
        let mut out = vec![];
        let mut marks: Vec<Mark> = vec![];
        let mut open: Vec<String> = vec![];
        while self.pos < self.toks.len() {
            match &self.toks[self.pos] {
                Tok::Text(t) => {
                    // Les sauts de ligne du HTML ne sont que des espaces.
                    push_text(&mut out, &t.replace(['\n', '\r'], " "), &marks);
                }
                Tok::Open(name, attrs) => match name.as_str() {
                    "strong" | "b" | "em" | "i" | "s" | "del" | "strike" | "code" => {
                        let m = match name.as_str() {
                            "strong" | "b" => "bold",
                            "em" | "i" => "italic",
                            "code" => "code",
                            _ => "strike",
                        };
                        marks.push(Mark { name: m, href: None });
                        open.push(name.clone());
                    }
                    "a" => {
                        let href = attr(attrs, "href").ok_or("a without href")?.to_string();
                        marks.push(Mark { name: "link", href: Some(href) });
                        open.push("a".into());
                    }
                    "span" if attr(attrs, "data-type") == Some("math-inline") => {
                        out.push(Inline::Math(attr(attrs, "latex").unwrap_or("").to_string()));
                        // Le contenu de la balise est ignoré (la source est dans `latex`).
                        let mut depth = 0;
                        while self.pos < self.toks.len() {
                            match &self.toks[self.pos] {
                                Tok::Open(n, _) if n == "span" => depth += 1,
                                Tok::Close(n) if n == "span" => {
                                    depth -= 1;
                                    if depth == 0 {
                                        break;
                                    }
                                }
                                _ => {}
                            }
                            self.pos += 1;
                        }
                    }
                    "br" if attr(attrs, "class") == Some("ProseMirror-trailingBreak") => {
                        // Saut de fin de paragraphe vide : sans effet. Sa fermeture suit.
                        self.pos += 1;
                    }
                    "p" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "ul" | "ol" | "li" | "blockquote" | "pre" | "hr" | "div" | "table" => return Ok(out),
                    other => return Err(other.to_string()),
                },
                Tok::Close(name) => {
                    if open.last() == Some(name) {
                        open.pop();
                        marks.pop();
                    } else if open.is_empty() {
                        return Ok(out);
                    } else {
                        return Err(format!("malformed </{name}>"));
                    }
                }
            }
            self.pos += 1;
        }
        Ok(out)
    }

    fn skip_ws(&mut self) {
        while let Some(Tok::Text(t)) = self.toks.get(self.pos) {
            if !t.trim().is_empty() {
                break;
            }
            self.pos += 1;
        }
    }

    /// Consomme la fermeture de `name`.
    fn close(&mut self, name: &str) -> Result<(), String> {
        self.skip_ws();
        match self.toks.get(self.pos) {
            Some(Tok::Close(n)) if n == name => {
                self.pos += 1;
                Ok(())
            }
            _ => Err(format!("unexpected content in <{name}>")),
        }
    }

    /// Texte brut jusqu'à la fermeture de `name` (blocs de code, sources de diagrammes), fermeture comprise.
    fn raw_text(&mut self, name: &str) -> Result<String, String> {
        let mut s = String::new();
        let mut depth = 1;
        while let Some(t) = self.toks.get(self.pos) {
            self.pos += 1;
            match t {
                Tok::Text(x) => s.push_str(x),
                Tok::Open(n, _) if n == name => depth += 1,
                Tok::Close(n) if n == name => {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(s);
                    }
                }
                _ => {}
            }
        }
        Err(format!("unclosed <{name}>"))
    }

    /// Blocs jusqu'à la fermeture non ouverte ici (non consommée) ou la fin.
    fn blocks(&mut self) -> Result<Vec<Block>, String> {
        let mut out = vec![];
        loop {
            self.skip_ws();
            let Some(tok) = self.toks.get(self.pos) else { return Ok(out) };
            let (name, attrs) = match tok {
                Tok::Close(_) => return Ok(out),
                Tok::Text(_) => {
                    let inl = self.inlines()?;
                    if !inl.is_empty() {
                        out.push(Block::Para(inl));
                    }
                    continue;
                }
                Tok::Open(n, a) => (n.clone(), a.clone()),
            };
            let kind = attr(&attrs, "data-type").unwrap_or("").to_string();
            // Mise en forme que l'éditeur ne sait pas reproduire en Markdown.
            if attr(&attrs, "style").is_some_and(|s| s.contains("text-align") || s.contains("color")) {
                return Err(format!("{name} with styling"));
            }
            match name.as_str() {
                "p" => {
                    self.pos += 1;
                    let inl = self.inlines()?;
                    self.close("p")?;
                    out.push(Block::Para(inl));
                }
                "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                    self.pos += 1;
                    let inl = self.inlines()?;
                    self.close(&name)?;
                    out.push(Block::Heading(name[1..].parse().unwrap_or(1), inl));
                }
                "strong" | "b" | "em" | "i" | "s" | "del" | "strike" | "code" | "a" | "span" => {
                    let inl = self.inlines()?;
                    out.push(Block::Para(inl));
                }
                "hr" => {
                    self.pos += 2;
                    out.push(Block::Rule);
                }
                "ul" | "ol" => {
                    if kind == "taskList" {
                        return Err("task list".into());
                    }
                    self.pos += 1;
                    let mut items = vec![];
                    loop {
                        self.skip_ws();
                        match self.toks.get(self.pos) {
                            Some(Tok::Open(n, _)) if n == "li" => {
                                self.pos += 1;
                                let inner = self.blocks()?;
                                self.close("li")?;
                                items.push(inner);
                            }
                            Some(Tok::Close(n)) if *n == name => {
                                self.pos += 1;
                                break;
                            }
                            _ => return Err(format!("unexpected content in <{name}>")),
                        }
                    }
                    out.push(if name == "ul" { Block::Bullet(items) } else { Block::Ordered(items) });
                }
                "blockquote" => {
                    self.pos += 1;
                    let inner = self.blocks()?;
                    self.close("blockquote")?;
                    out.push(if kind == "notice" { Block::Notice(attr(&attrs, "data-notice-type").unwrap_or("info").to_string(), inner) } else { Block::Quote(inner) });
                }
                "pre" => {
                    self.pos += 1;
                    let mut lang = None;
                    if let Some(Tok::Open(n, a)) = self.toks.get(self.pos) {
                        if n == "code" {
                            lang = attr(a, "class").and_then(|c| c.strip_prefix("language-")).map(str::to_string);
                            self.pos += 1;
                        }
                    }
                    let text = self.raw_text("pre")?;
                    out.push(Block::Code(lang, text.trim_end_matches('\n').to_string()));
                }
                "div" if kind == "math-block" => {
                    self.pos += 1;
                    let latex = attr(&attrs, "latex").unwrap_or("").to_string();
                    self.raw_text("div")?;
                    out.push(Block::Math(latex));
                }
                "div" if kind == "mermaid" || kind == "graph" => {
                    self.pos += 1;
                    let src = self.raw_text("div")?;
                    out.push(if kind == "mermaid" { Block::Mermaid(src) } else { Block::Graph(src) });
                }
                other => return Err(other.to_string()),
            }
        }
    }
}

/// Blocs d'un ancien HTML de page. `Err(balise)` si le HTML contient autre chose que ce que l'éditeur
/// sait écrire sans perte (images, tableaux, sauts de ligne forcés, couleurs, commentaires...).
pub fn html_to_blocks(html: &str) -> Result<Vec<Block>, String> {
    let mut p = HtmlParser { toks: tokenize(html), pos: 0 };
    let blocks = p.blocks()?;
    p.skip_ws();
    if p.pos < p.toks.len() {
        return Err("unbalanced HTML".into());
    }
    Ok(blocks)
}

/// État Yjs d'une page à partir de blocs (document neuf).
pub fn blocks_to_state(blocks: &[Block]) -> Vec<u8> {
    apply_blocks(&[], Op::Replace, blocks).expect("an empty document always loads")
}

