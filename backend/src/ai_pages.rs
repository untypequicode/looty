//! Pages <-> Markdown pour les agents IA.
//!
//! Le texte d'une page vit dans son état Yjs (`pages.yjs_state`, fragment « default » au
//! format de ProseMirror/Tiptap) ; `pages.content` n'est que l'ancien HTML, relu seulement
//! quand la page n'a jamais été ouverte dans l'éditeur. Les agents lisent et écrivent du
//! Markdown : ce module le convertit depuis et vers cet état Yjs, avec les mêmes nœuds que
//! l'éditeur (formules `mathBlock`/`mathInline`, `mermaid`, `graph`, encadrés `notice`…).
//! Écrire de l'Yjs (plutôt que remplacer le HTML) garde l'historique et l'édition
//! collaborative cohérents ; voir docs/adr/0010.

use std::collections::HashMap;
use std::sync::Arc;

use yrs::types::text::Diff;
use yrs::types::xml::{XmlElementPrelim, XmlFragment, XmlOut, XmlTextPrelim};
use yrs::updates::decoder::Decode;
use yrs::{Any, Doc, Options, OffsetKind, ReadTxn, StateVector, Text, Transact, Update, Xml};

const FRAGMENT: &str = "default";

/// Texte maximal renvoyé à un modèle pour une page (le reste est signalé comme coupé).
pub const MAX_READ_CHARS: usize = 60_000;

// ---------------------------------------------------------------------------
// Markdown -> blocs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct Mark {
    pub name: &'static str,
    pub href: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Inline {
    Text(String, Vec<Mark>),
    Math(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    Heading(u8, Vec<Inline>),
    Para(Vec<Inline>),
    Bullet(Vec<Vec<Block>>),
    Ordered(Vec<Vec<Block>>),
    Tasks(Vec<(bool, Vec<Block>)>),
    Code(Option<String>, String),
    Quote(Vec<Block>),
    Notice(String, Vec<Block>),
    Rule,
    Math(String),
    Mermaid(String),
    Graph(String),
    /// Première ligne = en-tête.
    Table(Vec<Vec<Vec<Inline>>>),
}

fn is_fence(l: &str) -> Option<(char, usize, &str)> {
    let t = l.trim_start();
    let c = t.chars().next()?;
    if c != '`' && c != '~' {
        return None;
    }
    let n = t.chars().take_while(|x| *x == c).count();
    if n < 3 {
        return None;
    }
    Some((c, n, t[n..].trim()))
}

fn heading_of(l: &str) -> Option<(u8, &str)> {
    let n = l.chars().take_while(|c| *c == '#').count();
    if (1..=6).contains(&n) && l[n..].starts_with(' ') {
        Some((n as u8, l[n..].trim().trim_end_matches('#').trim_end()))
    } else {
        None
    }
}

fn is_rule(l: &str) -> bool {
    let t = l.trim();
    t.len() >= 3 && ["-", "*", "_"].iter().any(|c| t.chars().all(|x| x.to_string() == *c || x == ' ') && t.matches(c).count() >= 3)
}

/// (indentation, ordonnée, reste après le marqueur)
fn list_marker(l: &str) -> Option<(usize, bool, &str)> {
    let indent = l.len() - l.trim_start().len();
    let t = l.trim_start();
    for m in ["- ", "* ", "+ "] {
        if let Some(r) = t.strip_prefix(m) {
            return Some((indent, false, r));
        }
    }
    let digits = t.chars().take_while(|c| c.is_ascii_digit()).count();
    if (1..=9).contains(&digits) && (t[digits..].starts_with(". ") || t[digits..].starts_with(") ")) {
        return Some((indent, true, &t[digits + 2..]));
    }
    None
}

fn is_table_sep(l: &str) -> bool {
    let t = l.trim();
    t.contains('-') && t.contains('|') && t.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
}

fn split_row(l: &str) -> Vec<String> {
    let t = l.trim().trim_start_matches('|').trim_end_matches('|');
    let mut cells = vec![];
    let mut cur = String::new();
    let mut chars = t.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' && chars.peek() == Some(&'|') {
            cur.push('|');
            chars.next();
        } else if c == '|' {
            cells.push(cur.trim().to_string());
            cur.clear();
        } else {
            cur.push(c);
        }
    }
    cells.push(cur.trim().to_string());
    cells
}

fn starts_block(l: &str) -> bool {
    is_fence(l).is_some()
        || heading_of(l).is_some()
        || l.trim_start().starts_with('>')
        || list_marker(l).is_some()
        || l.trim_start().starts_with("$$")
        || l.trim_start().starts_with(":::")
        || is_rule(l)
}

pub fn parse_markdown(md: &str) -> Vec<Block> {
    let lines: Vec<&str> = md.lines().collect();
    parse_blocks(&lines)
}

fn parse_blocks(lines: &[&str]) -> Vec<Block> {
    let mut out = vec![];
    let mut i = 0;
    while i < lines.len() {
        let l = lines[i];
        if l.trim().is_empty() {
            i += 1;
            continue;
        }
        if let Some((c, n, info)) = is_fence(l) {
            let mut body = vec![];
            i += 1;
            while i < lines.len() {
                let t = lines[i].trim();
                if t.chars().all(|x| x == c) && t.len() >= n {
                    break;
                }
                body.push(lines[i]);
                i += 1;
            }
            i += 1;
            let text = body.join("\n");
            let lang = info.split_whitespace().next().unwrap_or("").to_lowercase();
            out.push(match lang.as_str() {
                "mermaid" => Block::Mermaid(text),
                "looty-graph" | "graph" => Block::Graph(text),
                "math" => Block::Math(text),
                "" => Block::Code(None, text),
                _ => Block::Code(Some(lang), text),
            });
            continue;
        }
        let t = l.trim();
        if let Some(rest) = t.strip_prefix("$$") {
            if let Some(one) = rest.strip_suffix("$$").filter(|_| !rest.is_empty()) {
                out.push(Block::Math(one.trim().to_string()));
                i += 1;
                continue;
            }
            let mut body = vec![];
            if !rest.trim().is_empty() {
                body.push(rest.trim());
            }
            i += 1;
            while i < lines.len() && !lines[i].trim().ends_with("$$") {
                body.push(lines[i]);
                i += 1;
            }
            if i < lines.len() {
                let last = lines[i].trim().trim_end_matches("$$").trim();
                if !last.is_empty() {
                    body.push(last);
                }
            }
            i += 1;
            out.push(Block::Math(body.join("\n")));
            continue;
        }
        if let Some(kind) = t.strip_prefix(":::") {
            let kind = kind.trim();
            if !kind.is_empty() {
                let mut body = vec![];
                i += 1;
                while i < lines.len() && lines[i].trim() != ":::" {
                    body.push(lines[i]);
                    i += 1;
                }
                i += 1;
                let kind = if ["info", "success", "warning", "tip"].contains(&kind) { kind } else { "info" };
                out.push(Block::Notice(kind.to_string(), parse_blocks(&body)));
                continue;
            }
        }
        if let Some((level, text)) = heading_of(l) {
            out.push(Block::Heading(level, parse_inline(text)));
            i += 1;
            continue;
        }
        if is_rule(l) && list_marker(l).is_none_or(|(_, _, r)| r.trim().chars().all(|c| matches!(c, '-' | '*' | '_' | ' '))) {
            out.push(Block::Rule);
            i += 1;
            continue;
        }
        if t.starts_with('>') {
            let mut body = vec![];
            while i < lines.len() && lines[i].trim_start().starts_with('>') {
                let s = lines[i].trim_start().trim_start_matches('>');
                body.push(s.strip_prefix(' ').unwrap_or(s));
                i += 1;
            }
            out.push(Block::Quote(parse_blocks(&body)));
            continue;
        }
        if l.contains('|') && i + 1 < lines.len() && is_table_sep(lines[i + 1]) {
            let mut rows = vec![split_row(l)];
            i += 2;
            while i < lines.len() && lines[i].contains('|') && !lines[i].trim().is_empty() {
                rows.push(split_row(lines[i]));
                i += 1;
            }
            let width = rows[0].len();
            out.push(Block::Table(
                rows.into_iter()
                    .map(|mut r| {
                        r.resize(width, String::new());
                        r.iter().map(|c| parse_inline(c)).collect()
                    })
                    .collect(),
            ));
            continue;
        }
        if let Some((base, ordered, _)) = list_marker(l) {
            let mut items: Vec<Vec<String>> = vec![];
            while i < lines.len() {
                let cur = lines[i];
                if cur.trim().is_empty() {
                    // Ligne vide : la liste continue seulement si la suite est un item ou indentée.
                    let next = lines.get(i + 1).copied().unwrap_or("");
                    let more = list_marker(next).is_some_and(|(ind, o, _)| ind >= base && (ind > base || o == ordered))
                        || (!next.trim().is_empty() && next.len() - next.trim_start().len() > base);
                    if !more {
                        break;
                    }
                    if let Some(item) = items.last_mut() {
                        item.push(String::new());
                    }
                    i += 1;
                    continue;
                }
                match list_marker(cur) {
                    Some((ind, o, rest)) if ind == base && o == ordered => items.push(vec![rest.to_string()]),
                    Some((ind, _, _)) if ind <= base => break,
                    _ => {
                        let ind = cur.len() - cur.trim_start().len();
                        if ind <= base && !items.is_empty() && starts_block(cur) {
                            break;
                        }
                        if ind <= base && items.is_empty() {
                            break;
                        }
                        let strip = ind.min(base + 2).min(cur.len());
                        if let Some(item) = items.last_mut() {
                            item.push(cur[strip..].to_string());
                        }
                    }
                }
                i += 1;
            }
            let parsed: Vec<Vec<String>> = items;
            let task_of = |first: &str| -> Option<(bool, String)> {
                let f = first.trim_start();
                if let Some(r) = f.strip_prefix("[ ] ") {
                    Some((false, r.to_string()))
                } else if let Some(r) = f.strip_prefix("[x] ").or_else(|| f.strip_prefix("[X] ")) {
                    Some((true, r.to_string()))
                } else {
                    None
                }
            };
            let is_tasks = !ordered && parsed.first().and_then(|it| it.first()).is_some_and(|f| task_of(f).is_some());
            if is_tasks {
                out.push(Block::Tasks(
                    parsed
                        .into_iter()
                        .map(|mut it| {
                            let (done, first) = task_of(&it[0]).unwrap_or((false, it[0].clone()));
                            it[0] = first;
                            let refs: Vec<&str> = it.iter().map(String::as_str).collect();
                            (done, parse_blocks(&refs))
                        })
                        .collect(),
                ));
            } else {
                let blocks: Vec<Vec<Block>> = parsed
                    .into_iter()
                    .map(|it| {
                        let refs: Vec<&str> = it.iter().map(String::as_str).collect();
                        parse_blocks(&refs)
                    })
                    .collect();
                out.push(if ordered { Block::Ordered(blocks) } else { Block::Bullet(blocks) });
            }
            continue;
        }
        // Paragraphe : lignes jusqu'à la prochaine ligne vide ou le prochain bloc.
        let mut para = vec![t.to_string()];
        i += 1;
        while i < lines.len() && !lines[i].trim().is_empty() && !starts_block(lines[i]) && !(lines[i].contains('|') && i + 1 < lines.len() && is_table_sep(lines[i + 1])) {
            para.push(lines[i].trim().to_string());
            i += 1;
        }
        out.push(Block::Para(parse_inline(&para.join(" "))));
    }
    out
}

// ---------------------------------------------------------------------------
// Markdown en ligne
// ---------------------------------------------------------------------------

pub fn parse_inline(s: &str) -> Vec<Inline> {
    let chars: Vec<char> = s.chars().collect();
    let mut out = vec![];
    inline(&chars, &mut vec![], &mut out);
    out
}

fn find(chars: &[char], from: usize, pat: &str) -> Option<usize> {
    let p: Vec<char> = pat.chars().collect();
    (from..chars.len().saturating_sub(p.len() - 1)).find(|&i| chars[i..i + p.len()] == p[..])
}

fn push_text(out: &mut Vec<Inline>, text: &str, marks: &[Mark]) {
    if text.is_empty() {
        return;
    }
    if let Some(Inline::Text(prev, pm)) = out.last_mut() {
        if pm == marks {
            prev.push_str(text);
            return;
        }
    }
    out.push(Inline::Text(text.to_string(), marks.to_vec()));
}

fn inline(chars: &[char], marks: &mut Vec<Mark>, out: &mut Vec<Inline>) {
    let mut buf = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let prev_alnum = i > 0 && chars[i - 1].is_alphanumeric();
        if c == '\\' && i + 1 < chars.len() && chars[i + 1].is_ascii_punctuation() {
            buf.push(chars[i + 1]);
            i += 2;
            continue;
        }
        if c == '`' {
            if let Some(end) = find(chars, i + 1, "`") {
                if end > i + 1 {
                    push_text(out, &buf, marks);
                    buf.clear();
                    let mut m = marks.clone();
                    m.push(Mark { name: "code", href: None });
                    push_text(out, &chars[i + 1..end].iter().collect::<String>(), &m);
                    i = end + 1;
                    continue;
                }
            }
        }
        if c == '$' && i + 1 < chars.len() && !chars[i + 1].is_whitespace() && chars[i + 1] != '$' {
            // Formule en ligne : fermeture sans espace avant, non suivie d'un chiffre (« 5 $ et 6 $ »).
            let mut j = i + 1;
            let mut found = None;
            while j < chars.len() {
                if chars[j] == '$' && !chars[j - 1].is_whitespace() && chars[j - 1] != '\\' && !chars.get(j + 1).is_some_and(|x| x.is_ascii_digit()) {
                    found = Some(j);
                    break;
                }
                j += 1;
            }
            if let Some(end) = found {
                push_text(out, &buf, marks);
                buf.clear();
                out.push(Inline::Math(chars[i + 1..end].iter().collect()));
                i = end + 1;
                continue;
            }
        }
        if c == '[' {
            if let Some(close) = find(chars, i + 1, "](") {
                if let Some(end) = find(chars, close + 2, ")") {
                    push_text(out, &buf, marks);
                    buf.clear();
                    let href: String = chars[close + 2..end].iter().collect();
                    marks.push(Mark { name: "link", href: Some(href.trim().to_string()) });
                    inline(&chars[i + 1..close], marks, out);
                    marks.pop();
                    i = end + 1;
                    continue;
                }
            }
        }
        let mut consumed = false;
        for (pat, name) in [("**", "bold"), ("__", "bold"), ("~~", "strike"), ("*", "italic"), ("_", "italic")] {
            let p: Vec<char> = pat.chars().collect();
            if !chars[i..].starts_with(&p) || (pat == "_" && prev_alnum) || (p.len() == 1 && chars.get(i + 1) == Some(&c)) {
                continue;
            }
            if chars.get(i + p.len()).is_none_or(|a| a.is_whitespace()) {
                continue;
            }
            if let Some(end) = find(chars, i + p.len(), pat) {
                if end > i + p.len() && !chars[end - 1].is_whitespace() {
                    push_text(out, &buf, marks);
                    buf.clear();
                    marks.push(Mark { name, href: None });
                    inline(&chars[i + p.len()..end], marks, out);
                    marks.pop();
                    i = end + p.len();
                    consumed = true;
                    break;
                }
            }
        }
        if consumed {
            continue;
        }
        buf.push(c);
        i += 1;
    }
    push_text(out, &buf, marks);
}

// ---------------------------------------------------------------------------
// Blocs -> Yjs
// ---------------------------------------------------------------------------

type Attrs = HashMap<Arc<str>, Any>;

fn mark_attrs(marks: &[Mark]) -> Attrs {
    marks
        .iter()
        .map(|m| {
            let mut v: HashMap<String, Any> = HashMap::new();
            if let Some(h) = &m.href {
                v.insert("href".into(), Any::from(h.as_str()));
            }
            (Arc::from(m.name), Any::Map(Arc::new(v)))
        })
        .collect()
}

fn write_inlines<P: XmlFragment>(txn: &mut yrs::TransactionMut, parent: &P, inlines: &[Inline]) {
    let mut cur: Option<yrs::XmlTextRef> = None;
    let mut active: Vec<&'static str> = vec![];
    for inl in inlines {
        match inl {
            Inline::Text(t, marks) => {
                let text = match &cur {
                    Some(x) => x.clone(),
                    None => {
                        let x = parent.push_back(txn, XmlTextPrelim::new(""));
                        cur = Some(x.clone());
                        x
                    }
                };
                let at = text.len(txn);
                let mut attrs = mark_attrs(marks);
                // Le texte inséré hérite de la mise en forme de ce qui précède : on retire explicitement le reste.
                for name in active.iter() {
                    attrs.entry(Arc::from(*name)).or_insert(Any::Null);
                }
                active = marks.iter().map(|m| m.name).collect();
                if attrs.is_empty() {
                    text.insert(txn, at, t);
                } else {
                    text.insert_with_attributes(txn, at, t, attrs);
                }
            }
            Inline::Math(src) => {
                cur = None;
                active.clear();
                let el = parent.push_back(txn, XmlElementPrelim::empty("mathInline"));
                el.push_back(txn, XmlTextPrelim::new(src.as_str()));
            }
        }
    }
}

fn source_node<P: XmlFragment>(txn: &mut yrs::TransactionMut, parent: &P, tag: &str, src: &str) {
    let el = parent.push_back(txn, XmlElementPrelim::empty(tag));
    if !src.is_empty() {
        el.push_back(txn, XmlTextPrelim::new(src));
    }
}

fn write_blocks<P: XmlFragment>(txn: &mut yrs::TransactionMut, parent: &P, blocks: &[Block]) {
    for b in blocks {
        match b {
            Block::Heading(level, inl) => {
                let el = parent.push_back(txn, XmlElementPrelim::empty("heading"));
                el.insert_attribute(txn, "level", Any::from(*level as f64));
                write_inlines(txn, &el, inl);
            }
            Block::Para(inl) => {
                let el = parent.push_back(txn, XmlElementPrelim::empty("paragraph"));
                write_inlines(txn, &el, inl);
            }
            Block::Bullet(items) | Block::Ordered(items) => {
                let tag = if matches!(b, Block::Bullet(_)) { "bulletList" } else { "orderedList" };
                let list = parent.push_back(txn, XmlElementPrelim::empty(tag));
                for it in items {
                    let li = list.push_back(txn, XmlElementPrelim::empty("listItem"));
                    write_item(txn, &li, it);
                }
            }
            Block::Tasks(items) => {
                let list = parent.push_back(txn, XmlElementPrelim::empty("taskList"));
                for (done, it) in items {
                    let li = list.push_back(txn, XmlElementPrelim::empty("taskItem"));
                    li.insert_attribute(txn, "checked", Any::Bool(*done));
                    write_item(txn, &li, it);
                }
            }
            Block::Code(lang, text) => {
                let el = parent.push_back(txn, XmlElementPrelim::empty("codeBlock"));
                if let Some(l) = lang {
                    el.insert_attribute(txn, "language", l.as_str());
                }
                if !text.is_empty() {
                    el.push_back(txn, XmlTextPrelim::new(text.as_str()));
                }
            }
            Block::Quote(inner) => {
                let el = parent.push_back(txn, XmlElementPrelim::empty("blockquote"));
                write_blocks(txn, &el, inner);
                ensure_content(txn, &el);
            }
            Block::Notice(kind, inner) => {
                let el = parent.push_back(txn, XmlElementPrelim::empty("notice"));
                el.insert_attribute(txn, "type", kind.as_str());
                write_blocks(txn, &el, inner);
                ensure_content(txn, &el);
            }
            Block::Rule => {
                parent.push_back(txn, XmlElementPrelim::empty("horizontalRule"));
            }
            Block::Math(s) => source_node(txn, parent, "mathBlock", s),
            Block::Mermaid(s) => source_node(txn, parent, "mermaid", s),
            Block::Graph(s) => source_node(txn, parent, "graph", s),
            Block::Table(rows) => {
                let table = parent.push_back(txn, XmlElementPrelim::empty("table"));
                for (r, row) in rows.iter().enumerate() {
                    let tr = table.push_back(txn, XmlElementPrelim::empty("tableRow"));
                    for cell in row {
                        let td = tr.push_back(txn, XmlElementPrelim::empty(if r == 0 { "tableHeader" } else { "tableCell" }));
                        let p = td.push_back(txn, XmlElementPrelim::empty("paragraph"));
                        write_inlines(txn, &p, cell);
                    }
                }
            }
        }
    }
}

/// Les conteneurs du schéma exigent au moins un bloc.
fn ensure_content<P: XmlFragment>(txn: &mut yrs::TransactionMut, el: &P) {
    if el.len(txn) == 0 {
        el.push_back(txn, XmlElementPrelim::empty("paragraph"));
    }
}

fn write_item<P: XmlFragment>(txn: &mut yrs::TransactionMut, li: &P, blocks: &[Block]) {
    write_blocks(txn, li, blocks);
    ensure_content(txn, li);
}

// ---------------------------------------------------------------------------
// Yjs -> Markdown
// ---------------------------------------------------------------------------

fn attr_str<T: ReadTxn>(txn: &T, el: &yrs::XmlElementRef, name: &str) -> Option<String> {
    el.get_attribute(txn, name).map(|o| o.to_string(txn))
}

fn render_text<T: ReadTxn>(txn: &T, t: &yrs::XmlTextRef) -> String {
    let mut out = String::new();
    for d in t.diff(txn, |_| ()) as Vec<Diff<()>> {
        let text = d.insert.to_string(txn);
        let names: Vec<String> = d.attributes.as_ref().map(|a| a.keys().map(|k| k.split("--").next().unwrap_or(k).to_string()).collect()).unwrap_or_default();
        if text.is_empty() {
            continue;
        }
        let (lead, core, trail) = {
            let core = text.trim();
            if core.is_empty() {
                (text.as_str(), "", "")
            } else {
                let start = text.len() - text.trim_start().len();
                let end = text.trim_end().len();
                (&text[..start], core, &text[end..])
            }
        };
        let mut s = core.to_string();
        if !s.is_empty() {
            if names.iter().any(|n| n == "code") {
                s = format!("`{s}`");
            }
            if names.iter().any(|n| n == "bold") {
                s = format!("**{s}**");
            }
            if names.iter().any(|n| n == "italic") {
                s = format!("*{s}*");
            }
            if names.iter().any(|n| n == "strike") {
                s = format!("~~{s}~~");
            }
            if let Some(a) = d.attributes.as_ref() {
                if let Some((_, v)) = a.iter().find(|(k, _)| k.starts_with("link")) {
                    if let Any::Map(m) = v {
                        if let Some(Any::String(h)) = m.get("href") {
                            s = format!("[{s}]({h})");
                        }
                    }
                }
            }
        }
        out.push_str(lead);
        out.push_str(&s);
        out.push_str(trail);
    }
    out
}

fn render_inline<T: ReadTxn>(txn: &T, node: &impl XmlFragment) -> String {
    let mut out = String::new();
    for child in node.children(txn) {
        match child {
            XmlOut::Text(t) => out.push_str(&render_text(txn, &t)),
            XmlOut::Element(e) => match e.tag().as_ref() {
                "mathInline" => out.push_str(&format!("${}$", plain(txn, &e))),
                "hardBreak" => out.push_str("  \n"),
                other => {
                    let inner = plain(txn, &e);
                    out.push_str(&format!("[{other}{}]", if inner.is_empty() { String::new() } else { format!(": {inner}") }));
                }
            },
            XmlOut::Fragment(_) => {}
        }
    }
    out
}

/// Texte brut d'un nœud (sans les balises de `get_string`) : sources de formules, code, diagrammes.
fn plain<T: ReadTxn>(txn: &T, node: &impl XmlFragment) -> String {
    let mut out = String::new();
    for child in node.children(txn) {
        if let XmlOut::Text(t) = child {
            for d in t.diff(txn, |_| ()) as Vec<Diff<()>> {
                out.push_str(&d.insert.to_string(txn));
            }
        }
    }
    out
}

fn indent(s: &str, pad: &str) -> String {
    s.lines().map(|l| if l.is_empty() { String::new() } else { format!("{pad}{l}") }).collect::<Vec<_>>().join("\n")
}

fn render_children<T: ReadTxn>(txn: &T, node: &impl XmlFragment) -> String {
    let mut blocks = vec![];
    for child in node.children(txn) {
        if let XmlOut::Element(e) = child {
            let s = render_block(txn, &e);
            if !s.trim().is_empty() {
                blocks.push(s);
            }
        }
    }
    blocks.join("\n\n")
}

fn render_block<T: ReadTxn>(txn: &T, e: &yrs::XmlElementRef) -> String {
    match e.tag().as_ref() {
        "paragraph" => render_inline(txn, e),
        "heading" => {
            let n: usize = attr_str(txn, e, "level").and_then(|l| l.parse::<f64>().ok()).map(|f| f as usize).unwrap_or(1).clamp(1, 6);
            format!("{} {}", "#".repeat(n), render_inline(txn, e))
        }
        "bulletList" | "orderedList" | "taskList" => {
            let tag = e.tag().to_string();
            let mut out = vec![];
            for (n, child) in e.children(txn).enumerate() {
                if let XmlOut::Element(li) = child {
                    let body = render_children(txn, &li).replace("\n\n", "\n");
                    let marker = match tag.as_str() {
                        "orderedList" => format!("{}. ", n + 1),
                        "taskList" => format!("- [{}] ", if attr_str(txn, &li, "checked").as_deref() == Some("true") { "x" } else { " " }),
                        _ => "- ".to_string(),
                    };
                    let pad = " ".repeat(marker.len());
                    let mut lines = body.lines();
                    let first = lines.next().unwrap_or("");
                    let rest: Vec<&str> = lines.collect();
                    out.push(if rest.is_empty() { format!("{marker}{first}") } else { format!("{marker}{first}\n{}", indent(&rest.join("\n"), &pad)) });
                }
            }
            out.join("\n")
        }
        "blockquote" => indent(&render_children(txn, e), "> "),
        "notice" => format!(":::{}\n{}\n:::", attr_str(txn, e, "type").unwrap_or_else(|| "info".into()), render_children(txn, e)),
        "codeBlock" => format!("```{}\n{}\n```", attr_str(txn, e, "language").unwrap_or_default(), plain(txn, e)),
        "horizontalRule" => "---".to_string(),
        "mathBlock" => format!("$$\n{}\n$$", plain(txn, e)),
        "mermaid" => format!("```mermaid\n{}\n```", plain(txn, e)),
        "graph" => format!("```looty-graph\n{}\n```", plain(txn, e)),
        "table" => {
            let mut rows = vec![];
            for tr in e.children(txn) {
                if let XmlOut::Element(tr) = tr {
                    let cells: Vec<String> = tr
                        .children(txn)
                        .filter_map(|c| if let XmlOut::Element(c) = c { Some(render_children(txn, &c).replace('\n', " ").replace('|', "\\|")) } else { None })
                        .collect();
                    rows.push(format!("| {} |", cells.join(" | ")));
                }
            }
            if rows.len() > 1 {
                let n = rows[0].matches(" | ").count() + 1;
                rows.insert(1, format!("|{}", " --- |".repeat(n)));
            }
            rows.join("\n")
        }
        other => format!("[bloc non textuel : {other}]"),
    }
}

// ---------------------------------------------------------------------------
// API publique : état Yjs <-> Markdown
// ---------------------------------------------------------------------------

fn new_doc() -> Doc {
    Doc::with_options(Options { offset_kind: OffsetKind::Utf16, ..Options::default() })
}

fn load(state: &[u8]) -> Result<Doc, String> {
    let doc = new_doc();
    if !state.is_empty() {
        let update = Update::decode_v1(state).map_err(|e| e.to_string())?;
        doc.transact_mut().apply_update(update).map_err(|e| e.to_string())?;
    }
    Ok(doc)
}

/// Markdown d'un état Yjs de page.
pub fn state_to_markdown(state: &[u8]) -> Result<String, String> {
    let doc = load(state)?;
    let frag = doc.get_or_insert_xml_fragment(FRAGMENT);
    let txn = doc.transact();
    Ok(render_children(&txn, &frag))
}

/// Texte approximatif de l'ancien HTML (pages jamais ouvertes dans l'éditeur).
pub fn html_to_text(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    let mut tag = String::new();
    for c in html.chars() {
        match c {
            '<' => {
                in_tag = true;
                tag.clear();
            }
            '>' if in_tag => {
                in_tag = false;
                let t = tag.trim_start_matches('/').split_whitespace().next().unwrap_or("").to_lowercase();
                if matches!(t.as_str(), "p" | "div" | "br" | "li" | "h1" | "h2" | "h3" | "h4" | "tr") {
                    out.push('\n');
                }
            }
            _ if in_tag => tag.push(c),
            _ => out.push(c),
        }
    }
    out.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&nbsp;", " ").replace("&quot;", "\"").replace("&#39;", "'")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Append,
    Replace,
}

/// Applique du Markdown à un état Yjs ; renvoie le nouvel état complet.
pub fn apply_markdown(state: &[u8], op: Op, md: &str) -> Result<Vec<u8>, String> {
    let doc = load(state)?;
    let frag = doc.get_or_insert_xml_fragment(FRAGMENT);
    let blocks = parse_markdown(md);
    {
        let mut txn = doc.transact_mut();
        match op {
            Op::Replace => {
                let n = frag.len(&txn);
                if n > 0 {
                    frag.remove_range(&mut txn, 0, n);
                }
            }
            Op::Append => {
                // Le paragraphe vide final (nœud de fin de l'éditeur) est remplacé par l'ajout.
                let n = frag.len(&txn);
                if n > 0 {
                    if let Some(XmlOut::Element(last)) = frag.get(&txn, n - 1) {
                        if last.tag().as_ref() == "paragraph" && last.len(&txn) == 0 {
                            frag.remove_range(&mut txn, n - 1, 1);
                        }
                    }
                }
            }
        }
        write_blocks(&mut txn, &frag, &blocks);
        if frag.len(&txn) == 0 {
            frag.push_back(&mut txn, XmlElementPrelim::empty("paragraph"));
        }
    }
    let txn = doc.transact();
    Ok(txn.encode_state_as_update_v1(&StateVector::default()))
}

// ---------------------------------------------------------------------------
// Différences
// ---------------------------------------------------------------------------

/// Différence ligne à ligne (plus longue sous-suite commune) : (« + », « - » ou « » , ligne).
pub fn diff_lines(before: &str, after: &str) -> Vec<(char, String)> {
    let a: Vec<&str> = before.lines().collect();
    let b: Vec<&str> = after.lines().collect();
    let (n, m) = (a.len(), b.len());
    let mut lcs = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if a[i] == b[j] { lcs[i + 1][j + 1] + 1 } else { lcs[i + 1][j].max(lcs[i][j + 1]) };
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut out = vec![];
    while i < n && j < m {
        if a[i] == b[j] {
            out.push((' ', a[i].to_string()));
            i += 1;
            j += 1;
        } else if lcs[i + 1][j] >= lcs[i][j + 1] {
            out.push(('-', a[i].to_string()));
            i += 1;
        } else {
            out.push(('+', b[j].to_string()));
            j += 1;
        }
    }
    out.extend(a[i..].iter().map(|l| ('-', l.to_string())));
    out.extend(b[j..].iter().map(|l| ('+', l.to_string())));
    out
}

/// Syntaxe acceptée par les pages, donnée au modèle dans son invite système.
pub const SYNTAX_GUIDE: &str = "\
Page content is Markdown. Supported syntax (anything else is plain text):
- Headings `# ` to `###### `, paragraphs, **bold**, *italic*, ~~strike~~, `inline code`, [links](https://...).
- Lists: `- item`, `1. item`, task lists `- [ ] todo` / `- [x] done`; nest with 2-space indentation.
- `> quote`, `---` rule, fenced code blocks with a language (```python).
- Tables in GitHub style (`| a | b |` then `| --- | --- |`), one line per row, no block content in cells.
- Callouts: `:::info`, `:::success`, `:::warning` or `:::tip` on its own line, content, then `:::`.
- Maths in LaTeX: inline `$x^2$` (no space after the opening or before the closing `$`), block `$$` on its own lines, or a ```math fence. Never put maths in code spans.
- Diagrams: a ```mermaid fence (flowcharts, sequence, Gantt...) or a ```looty-graph fence for weighted/undirected graphs, one line per node or edge: `s : Source`, `s -> t : 3/5`, `a -- b : 2`, a trailing `*` highlights; positions are optional (`s : Source @ 0 100`).
- Not supported from here: images, embeds, boards, comments. Existing ones appear as `[bloc non textuel : ...]` when reading: never rewrite them, prefer `append`.";

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(md: &str) -> String {
        let state = apply_markdown(&[], Op::Replace, md).unwrap();
        state_to_markdown(&state).unwrap()
    }

    #[test]
    fn headings_lists_and_marks_roundtrip() {
        let md = "# Titre\n\nUn **gras**, un *italique* et du `code`.\n\n- a\n- b\n\n1. un\n2. deux\n\n- [x] fait\n- [ ] à faire";
        assert_eq!(roundtrip(md), md);
    }

    #[test]
    fn maths_diagrams_and_callouts_roundtrip() {
        let md = "Soit $x^2$ ici.\n\n$$\n\\int_0^1 x\\,dx\n$$\n\n```mermaid\ngraph TD\nA-->B\n```\n\n```looty-graph\ns -> t : 3/5\n```\n\n:::warning\nattention\n:::";
        assert_eq!(roundtrip(md), md);
    }

    #[test]
    fn currency_is_not_maths_and_links_survive() {
        let md = "Coûte 5 $ ou 6 $ ; voir [le site](https://example.org).";
        assert_eq!(roundtrip(md), md);
    }

    #[test]
    fn tables_and_code_roundtrip() {
        let md = "| a | b |\n| --- | --- |\n| 1 | 2 |\n\n```python\nprint(1)\n```";
        assert_eq!(roundtrip(md), md);
    }

    #[test]
    fn append_keeps_existing_content_and_replace_drops_it() {
        let s1 = apply_markdown(&[], Op::Replace, "premier").unwrap();
        let s2 = apply_markdown(&s1, Op::Append, "second").unwrap();
        assert_eq!(state_to_markdown(&s2).unwrap(), "premier\n\nsecond");
        let s3 = apply_markdown(&s2, Op::Replace, "seul").unwrap();
        assert_eq!(state_to_markdown(&s3).unwrap(), "seul");
    }

    #[test]
    fn diff_marks_changed_lines() {
        let d = diff_lines("a\nb\nc", "a\nx\nc");
        assert_eq!(d, vec![(' ', "a".into()), ('-', "b".into()), ('+', "x".into()), (' ', "c".into())]);
    }
}
