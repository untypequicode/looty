//! Fournisseurs de modèles : un trait, une implémentation par famille d'API, réponses en flux.
//!
//! OpenAI, Perplexity et les serveurs compatibles (Ollama, vLLM, LM Studio…) parlent la même
//! API « chat/completions » ; Anthropic et Gemini ont la leur. Chaque adaptateur traduit
//! l'historique commun (`Msg`) vers son format et renvoie des `StreamEvent`.

use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::sync::mpsc::Sender;

#[derive(Clone, Debug, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub args: Value,
    /// Signature de pensée de Gemini 3 (`thoughtSignature`) : à renvoyer telle quelle avec l'appel.
    pub signature: Option<String>,
}

/// Signature factice documentée par Google, pour les appels enregistrés sans signature.
pub const DUMMY_SIGNATURE: &str = "skip_thought_signature_validator";

/// Historique commun à tous les fournisseurs.
#[derive(Clone, Debug)]
pub enum Msg {
    User(String),
    Assistant { text: String, calls: Vec<ToolCall> },
    ToolResult { call_id: String, name: String, content: String },
}

#[derive(Clone, Debug)]
pub struct ToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    pub parameters: Value,
}

pub struct ChatRequest<'a> {
    pub system: &'a str,
    pub messages: &'a [Msg],
    pub tools: &'a [ToolSpec],
}

#[derive(Debug)]
pub enum StreamEvent {
    Text(String),
    Call(ToolCall),
}

#[async_trait]
pub trait Provider: Send + Sync {
    /// Envoie la requête et pousse les morceaux de réponse dans `tx` au fil de l'eau.
    async fn stream(&self, req: &ChatRequest<'_>, tx: &Sender<StreamEvent>) -> Result<(), String>;
}

pub struct Conn {
    pub provider: String,
    pub model: String,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    /// Cible interne (réseau privé) : le corps des erreurs du fournisseur n'est jamais renvoyé.
    pub quiet: bool,
}

pub fn default_base_url(provider: &str) -> Option<&'static str> {
    match provider {
        "openai" => Some("https://api.openai.com/v1"),
        "anthropic" => Some("https://api.anthropic.com/v1"),
        "gemini" => Some("https://generativelanguage.googleapis.com/v1beta"),
        "perplexity" => Some("https://api.perplexity.ai"),
        _ => None,
    }
}

/// Les modèles Perplexity (Sonar) cherchent sur le web mais n'appellent pas d'outils.
pub fn supports_tools(provider: &str) -> bool {
    provider != "perplexity"
}

pub fn build(conn: &Conn) -> Result<Box<dyn Provider>, String> {
    let base = conn
        .base_url
        .clone()
        .filter(|b| !b.trim().is_empty())
        .or_else(|| default_base_url(&conn.provider).map(String::from))
        .ok_or("missing_base_url")?
        .trim_end_matches('/')
        .to_string();
    let c = Common { base, model: conn.model.clone(), key: conn.api_key.clone().unwrap_or_default(), quiet: conn.quiet };
    Ok(match conn.provider.as_str() {
        "openai" | "perplexity" | "openai_compatible" => Box::new(OpenAiLike(c)),
        "anthropic" => Box::new(Anthropic(c)),
        "gemini" => Box::new(Gemini(c)),
        other => return Err(format!("unknown_provider:{other}")),
    })
}

struct Common {
    base: String,
    model: String,
    key: String,
    quiet: bool,
}

fn client() -> Result<reqwest::Client, String> {
    // Pas de redirection : une redirection pourrait mener vers une adresse interne.
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(std::time::Duration::from_secs(15))
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| e.to_string())
}

/// Lit une réponse « server-sent events » ; `on_data` reçoit le contenu de chaque ligne `data:`.
async fn read_sse(mut resp: reqwest::Response, tx: &Sender<StreamEvent>, mut on_data: impl FnMut(&str, &mut Vec<StreamEvent>) -> Result<(), String>) -> Result<(), String> {
    // Octets : un caractère UTF-8 peut être coupé entre deux morceaux réseau.
    let mut buf: Vec<u8> = vec![];
    let mut out: Vec<StreamEvent> = vec![];
    loop {
        let chunk = resp.chunk().await.map_err(|e| e.without_url().to_string())?;
        let last = chunk.is_none();
        if let Some(c) = chunk {
            buf.extend_from_slice(&c);
        } else if !buf.is_empty() {
            buf.push(b'\n');
        }
        while let Some(pos) = buf.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = buf.drain(..=pos).collect();
            if let Some(d) = String::from_utf8_lossy(&line).trim().strip_prefix("data:") {
                on_data(d.trim(), &mut out)?;
            }
        }
        for e in out.drain(..) {
            tx.send(e).await.map_err(|_| "client_gone".to_string())?;
        }
        if last {
            return Ok(());
        }
    }
}

fn transport_error(e: reqwest::Error, quiet: bool) -> String {
    if quiet {
        "provider_unreachable".into()
    } else {
        e.without_url().to_string()
    }
}

/// Erreur HTTP lisible, sans jamais répéter la clé.
async fn http_error(resp: reqwest::Response, key: &str, quiet: bool) -> String {
    let status = resp.status();
    if quiet {
        return format!("provider_http_{}", status.as_u16());
    }
    let mut body = resp.text().await.unwrap_or_default();
    if !key.is_empty() {
        body = body.replace(key, "***");
    }
    let body: String = body.chars().take(400).collect();
    format!("provider_http_{}: {}", status.as_u16(), body)
}

// ---------------------------------------------------------------------------
// OpenAI et compatibles
// ---------------------------------------------------------------------------

struct OpenAiLike(Common);

fn openai_messages(req: &ChatRequest<'_>, gemini: bool) -> Vec<Value> {
    let mut v = vec![json!({ "role": "system", "content": req.system })];
    for m in req.messages {
        v.push(match m {
            Msg::User(t) => json!({ "role": "user", "content": t }),
            Msg::Assistant { text, calls } if calls.is_empty() => json!({ "role": "assistant", "content": text }),
            Msg::Assistant { text, calls } => json!({
                "role": "assistant",
                "content": if text.is_empty() { Value::Null } else { json!(text) },
                "tool_calls": calls.iter().map(|c| {
                    let mut tc = json!({
                        "id": c.id, "type": "function",
                        "function": { "name": c.name, "arguments": c.args.to_string() }
                    });
                    // Couche compatible OpenAI de Gemini : la signature voyage dans `extra_content`.
                    let sig = c.signature.as_deref().or(gemini.then_some(DUMMY_SIGNATURE));
                    if let Some(sig) = sig {
                        tc["extra_content"] = json!({ "google": { "thought_signature": sig } });
                    }
                    tc
                }).collect::<Vec<_>>(),
            }),
            Msg::ToolResult { call_id, content, .. } => json!({ "role": "tool", "tool_call_id": call_id, "content": content }),
        });
    }
    v
}

/// Assemble les appels d'outils d'OpenAI, envoyés en morceaux d'arguments indexés.
#[derive(Default)]
struct OpenAiCalls(Vec<(String, String, String, Option<String>)>);

impl OpenAiCalls {
    fn feed(&mut self, delta: &Value) {
        for tc in delta["tool_calls"].as_array().into_iter().flatten() {
            let idx = tc["index"].as_u64().unwrap_or(0) as usize;
            while self.0.len() <= idx {
                self.0.push(Default::default());
            }
            let slot = &mut self.0[idx];
            if let Some(id) = tc["id"].as_str() {
                slot.0 = id.to_string();
            }
            if let Some(n) = tc["function"]["name"].as_str() {
                slot.1.push_str(n);
            }
            if let Some(a) = tc["function"]["arguments"].as_str() {
                slot.2.push_str(a);
            }
            if let Some(sig) = tc["extra_content"]["google"]["thought_signature"].as_str().filter(|s| !s.is_empty()) {
                slot.3 = Some(sig.to_string());
            }
        }
    }
    fn finish(self) -> Vec<ToolCall> {
        self.0
            .into_iter()
            .filter(|(_, n, _, _)| !n.is_empty())
            .enumerate()
            .map(|(i, (id, name, args, signature))| ToolCall {
                id: if id.is_empty() { format!("call_{i}") } else { id },
                name,
                args: serde_json::from_str(&args).unwrap_or(json!({})),
                signature,
            })
            .collect()
    }
}

/// Point d'entrée Gemini (natif ou couche compatible OpenAI) : exige les signatures de pensée.
fn is_gemini_url(base: &str) -> bool {
    base.contains("generativelanguage.googleapis.com")
}

#[async_trait]
impl Provider for OpenAiLike {
    async fn stream(&self, req: &ChatRequest<'_>, tx: &Sender<StreamEvent>) -> Result<(), String> {
        let mut body = json!({ "model": self.0.model, "messages": openai_messages(req, is_gemini_url(&self.0.base)), "stream": true });
        if !req.tools.is_empty() {
            body["tools"] = json!(req
                .tools
                .iter()
                .map(|t| json!({ "type": "function", "function": { "name": t.name, "description": t.description, "parameters": t.parameters } }))
                .collect::<Vec<_>>());
        }
        let mut rb = client()?.post(format!("{}/chat/completions", self.0.base)).json(&body);
        if !self.0.key.is_empty() {
            rb = rb.bearer_auth(&self.0.key);
        }
        let resp = rb.send().await.map_err(|e| transport_error(e, self.0.quiet))?;
        if !resp.status().is_success() {
            return Err(http_error(resp, &self.0.key, self.0.quiet).await);
        }
        let mut calls = OpenAiCalls::default();
        read_sse(resp, tx, |d, out| {
            if d == "[DONE]" {
                return Ok(());
            }
            let v: Value = serde_json::from_str(d).map_err(|e| e.to_string())?;
            let delta = &v["choices"][0]["delta"];
            if let Some(t) = delta["content"].as_str().filter(|t| !t.is_empty()) {
                out.push(StreamEvent::Text(t.to_string()));
            }
            calls.feed(delta);
            Ok(())
        })
        .await?;
        for c in calls.finish() {
            tx.send(StreamEvent::Call(c)).await.map_err(|_| "client_gone".to_string())?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Anthropic
// ---------------------------------------------------------------------------

struct Anthropic(Common);

fn anthropic_messages(req: &ChatRequest<'_>) -> Vec<Value> {
    let mut v: Vec<Value> = vec![];
    for m in req.messages {
        match m {
            Msg::User(t) => v.push(json!({ "role": "user", "content": t })),
            Msg::Assistant { text, calls } => {
                let mut blocks = vec![];
                if !text.is_empty() {
                    blocks.push(json!({ "type": "text", "text": text }));
                }
                for c in calls {
                    blocks.push(json!({ "type": "tool_use", "id": c.id, "name": c.name, "input": c.args }));
                }
                if !blocks.is_empty() {
                    v.push(json!({ "role": "assistant", "content": blocks }));
                }
            }
            Msg::ToolResult { call_id, content, .. } => {
                let block = json!({ "type": "tool_result", "tool_use_id": call_id, "content": content });
                // Les résultats d'un même tour vont dans un seul message utilisateur.
                match v.last_mut() {
                    Some(last) if last["role"] == "user" && last["content"].is_array() => last["content"].as_array_mut().unwrap().push(block),
                    _ => v.push(json!({ "role": "user", "content": [block] })),
                }
            }
        }
    }
    v
}

#[async_trait]
impl Provider for Anthropic {
    async fn stream(&self, req: &ChatRequest<'_>, tx: &Sender<StreamEvent>) -> Result<(), String> {
        let mut body = json!({
            "model": self.0.model, "max_tokens": 8192, "stream": true,
            "system": req.system, "messages": anthropic_messages(req),
        });
        if !req.tools.is_empty() {
            body["tools"] = json!(req.tools.iter().map(|t| json!({ "name": t.name, "description": t.description, "input_schema": t.parameters })).collect::<Vec<_>>());
        }
        let resp = client()?
            .post(format!("{}/messages", self.0.base))
            .header("x-api-key", &self.0.key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await
            .map_err(|e| transport_error(e, self.0.quiet))?;
        if !resp.status().is_success() {
            return Err(http_error(resp, &self.0.key, self.0.quiet).await);
        }
        // Bloc d'outil en cours : (id, nom, JSON partiel des arguments).
        let mut tool: Option<(String, String, String)> = None;
        read_sse(resp, tx, |d, events| {
            let v: Value = serde_json::from_str(d).map_err(|e| e.to_string())?;
            match v["type"].as_str() {
                Some("content_block_start") if v["content_block"]["type"] == "tool_use" => {
                    tool = Some((v["content_block"]["id"].as_str().unwrap_or("").into(), v["content_block"]["name"].as_str().unwrap_or("").into(), String::new()));
                }
                Some("content_block_delta") => match v["delta"]["type"].as_str() {
                    Some("text_delta") => events.push(StreamEvent::Text(v["delta"]["text"].as_str().unwrap_or("").into())),
                    Some("input_json_delta") => {
                        if let Some(t) = tool.as_mut() {
                            t.2.push_str(v["delta"]["partial_json"].as_str().unwrap_or(""));
                        }
                    }
                    _ => {}
                },
                Some("content_block_stop") => {
                    if let Some((id, name, args)) = tool.take() {
                        events.push(StreamEvent::Call(ToolCall { id, name, args: serde_json::from_str(&args).unwrap_or(json!({})), signature: None }));
                    }
                }
                Some("error") => return Err(format!("provider_error: {}", v["error"]["message"].as_str().unwrap_or("unknown"))),
                _ => {}
            }
            Ok(())
        })
        .await
    }
}

// ---------------------------------------------------------------------------
// Gemini
// ---------------------------------------------------------------------------

struct Gemini(Common);

fn gemini_contents(req: &ChatRequest<'_>) -> Vec<Value> {
    let mut v: Vec<Value> = vec![];
    for m in req.messages {
        match m {
            Msg::User(t) => v.push(json!({ "role": "user", "parts": [{ "text": t }] })),
            Msg::Assistant { text, calls } => {
                let mut parts = vec![];
                if !text.is_empty() {
                    parts.push(json!({ "text": text }));
                }
                for c in calls {
                    // Sans signature (appel enregistré avant son support), la valeur factice évite le refus de Gemini 3.
                    let sig = c.signature.as_deref().unwrap_or(DUMMY_SIGNATURE);
                    parts.push(json!({ "functionCall": { "name": c.name, "args": c.args }, "thoughtSignature": sig }));
                }
                if !parts.is_empty() {
                    v.push(json!({ "role": "model", "parts": parts }));
                }
            }
            Msg::ToolResult { name, content, .. } => {
                let part = json!({ "functionResponse": { "name": name, "response": { "result": content } } });
                match v.last_mut() {
                    Some(last) if last["role"] == "user" && last["parts"][0]["functionResponse"].is_object() => last["parts"].as_array_mut().unwrap().push(part),
                    _ => v.push(json!({ "role": "user", "parts": [part] })),
                }
            }
        }
    }
    v
}

/// Événements d'un morceau de réponse Gemini (texte et appels, avec leur `thoughtSignature`).
fn gemini_events(v: &Value, events: &mut Vec<StreamEvent>) {
    for p in v["candidates"][0]["content"]["parts"].as_array().into_iter().flatten() {
        if let Some(t) = p["text"].as_str() {
            events.push(StreamEvent::Text(t.to_string()));
        }
        if p["functionCall"].is_object() {
            events.push(StreamEvent::Call(ToolCall {
                id: format!("call_{}", uuid::Uuid::new_v4().simple()),
                name: p["functionCall"]["name"].as_str().unwrap_or("").to_string(),
                args: p["functionCall"]["args"].clone(),
                signature: p["thoughtSignature"].as_str().filter(|s| !s.is_empty()).map(String::from),
            }));
        }
    }
}

#[async_trait]
impl Provider for Gemini {
    async fn stream(&self, req: &ChatRequest<'_>, tx: &Sender<StreamEvent>) -> Result<(), String> {
        let mut body = json!({
            "systemInstruction": { "parts": [{ "text": req.system }] },
            "contents": gemini_contents(req),
        });
        if !req.tools.is_empty() {
            body["tools"] = json!([{ "functionDeclarations": req.tools.iter().map(|t| json!({ "name": t.name, "description": t.description, "parameters": t.parameters })).collect::<Vec<_>>() }]);
        }
        let resp = client()?
            .post(format!("{}/models/{}:streamGenerateContent?alt=sse", self.0.base, self.0.model))
            .header("x-goog-api-key", &self.0.key)
            .json(&body)
            .send()
            .await
            .map_err(|e| transport_error(e, self.0.quiet))?;
        if !resp.status().is_success() {
            return Err(http_error(resp, &self.0.key, self.0.quiet).await);
        }
        read_sse(resp, tx, |d, events| {
            let v: Value = serde_json::from_str(d).map_err(|e| e.to_string())?;
            gemini_events(&v, events);
            Ok(())
        })
        .await
    }
}

// ---------------------------------------------------------------------------
// Liste des modèles (« Charger les modèles »)
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq, serde::Serialize)]
pub struct ModelInfo {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// Modèles qui ne font pas de discussion (embeddings, voix, images…) : écartés de la liste.
fn not_chat(id: &str) -> bool {
    const SKIP: [&str; 16] = [
        "embed", "whisper", "tts", "dall-e", "moderation", "image", "audio", "realtime", "transcribe", "davinci", "babbage", "sora", "imagen", "veo", "aqa", "rerank",
    ];
    let id = id.to_lowercase();
    SKIP.iter().any(|s| id.contains(s))
}

/// Extrait de la réponse « liste des modèles » d'un fournisseur les modèles de discussion, sans doublon.
/// Anthropic renvoie déjà le plus récent en premier : son ordre est gardé ; les autres sont triés par
/// identifiant décroissant (les versions les plus récentes d'abord).
pub fn parse_models(provider: &str, v: &Value) -> Vec<ModelInfo> {
    let mut out: Vec<ModelInfo> = vec![];
    let mut push = |id: &str, label: Option<&str>| {
        if !id.is_empty() && !not_chat(id) && !out.iter().any(|m| m.id == id) {
            out.push(ModelInfo { id: id.to_string(), label: label.filter(|l| !l.is_empty() && *l != id).map(String::from) });
        }
    };
    match provider {
        "gemini" => {
            for m in v["models"].as_array().into_iter().flatten() {
                let can_chat = m["supportedGenerationMethods"].as_array().is_none_or(|a| a.iter().any(|x| x == "generateContent"));
                if let (true, Some(name)) = (can_chat, m["name"].as_str()) {
                    push(name.trim_start_matches("models/"), m["displayName"].as_str());
                }
            }
        }
        "anthropic" => {
            for m in v["data"].as_array().into_iter().flatten() {
                if let Some(id) = m["id"].as_str() {
                    push(id, m["display_name"].as_str());
                }
            }
        }
        _ => {
            for m in v["data"].as_array().into_iter().flatten() {
                if let Some(id) = m["id"].as_str() {
                    push(id, None);
                }
            }
        }
    }
    if provider != "anthropic" {
        out.sort_by(|a, b| b.id.cmp(&a.id));
    }
    out
}

/// Interroge le point d'entrée « liste des modèles » du fournisseur. Erreurs au format de `stream`.
pub async fn list_models(conn: &Conn) -> Result<Vec<ModelInfo>, String> {
    let base = conn
        .base_url
        .clone()
        .filter(|b| !b.trim().is_empty())
        .or_else(|| default_base_url(&conn.provider).map(String::from))
        .ok_or("missing_base_url")?
        .trim_end_matches('/')
        .to_string();
    let key = conn.api_key.clone().unwrap_or_default();
    let c = client()?;
    let rb = match conn.provider.as_str() {
        "openai" | "openai_compatible" => {
            let rb = c.get(format!("{base}/models"));
            if key.is_empty() { rb } else { rb.bearer_auth(&key) }
        }
        "anthropic" => c.get(format!("{base}/models?limit=100")).header("x-api-key", &key).header("anthropic-version", "2023-06-01"),
        "gemini" => c.get(format!("{base}/models?pageSize=200")).header("x-goog-api-key", &key),
        _ => return Err("models_unsupported".into()),
    };
    let resp = rb.send().await.map_err(|e| transport_error(e, conn.quiet))?;
    if !resp.status().is_success() {
        return Err(http_error(resp, &key, conn.quiet).await);
    }
    let v: Value = resp.json().await.map_err(|_| "provider_bad_response".to_string())?;
    Ok(parse_models(&conn.provider, &v))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_lists_are_filtered_and_sorted() {
        let openai = json!({ "data": [
            { "id": "gpt-5.1" }, { "id": "text-embedding-3-small" }, { "id": "gpt-6" }, { "id": "whisper-1" }, { "id": "gpt-5.1" }, { "id": "gpt-image-1" }
        ] });
        let ids: Vec<_> = parse_models("openai", &openai).into_iter().map(|m| m.id).collect();
        assert_eq!(ids, ["gpt-6", "gpt-5.1"]);
        let anth = json!({ "data": [{ "id": "claude-b", "display_name": "B" }, { "id": "claude-a", "display_name": "claude-a" }] });
        let m = parse_models("anthropic", &anth);
        assert_eq!(m.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(), ["claude-b", "claude-a"]);
        assert_eq!(m[0].label.as_deref(), Some("B"));
        assert_eq!(m[1].label, None);
        let gem = json!({ "models": [
            { "name": "models/gemini-3-flash-preview", "displayName": "Gemini 3 Flash", "supportedGenerationMethods": ["generateContent"] },
            { "name": "models/text-embedding-004", "supportedGenerationMethods": ["embedContent"] },
            { "name": "models/aqa", "supportedGenerationMethods": ["generateAnswer"] },
            { "name": "models/gemini-3.1-pro-preview", "supportedGenerationMethods": ["generateContent", "countTokens"] }
        ] });
        let ids: Vec<_> = parse_models("gemini", &gem).into_iter().map(|m| m.id).collect();
        assert_eq!(ids, ["gemini-3.1-pro-preview", "gemini-3-flash-preview"]);
        assert!(parse_models("openai", &json!({ "oops": 1 })).is_empty());
    }

    #[test]
    fn openai_tool_call_arguments_are_assembled_across_chunks() {
        let mut c = OpenAiCalls::default();
        c.feed(&json!({ "tool_calls": [{ "index": 0, "id": "c1", "function": { "name": "read_page", "arguments": "{\"page" } }] }));
        c.feed(&json!({ "tool_calls": [{ "index": 0, "function": { "arguments": "_id\":\"x\"}" } }] }));
        let calls = c.finish();
        assert_eq!(calls, vec![ToolCall { id: "c1".into(), name: "read_page".into(), args: json!({ "page_id": "x" }), signature: None }]);
    }

    fn call(sig: Option<&str>) -> ToolCall {
        ToolCall { id: "c1".into(), name: "read_page".into(), args: json!({ "a": 1 }), signature: sig.map(String::from) }
    }

    #[test]
    fn gemini_signature_is_captured_and_replayed() {
        let chunk = json!({ "candidates": [{ "content": { "parts": [
            { "text": "ok" },
            { "functionCall": { "name": "read_page", "args": { "a": 1 } }, "thoughtSignature": "SIG123" },
        ] } }] });
        let mut ev = vec![];
        gemini_events(&chunk, &mut ev);
        let StreamEvent::Call(c) = &ev[1] else { panic!("call expected") };
        assert_eq!(c.signature.as_deref(), Some("SIG123"));
        let msgs = vec![Msg::User("q".into()), Msg::Assistant { text: String::new(), calls: vec![c.clone()] }];
        let out = gemini_contents(&ChatRequest { system: "s", messages: &msgs, tools: &[] });
        assert_eq!(out[1]["parts"][0]["thoughtSignature"], "SIG123");
    }

    #[test]
    fn gemini_old_calls_without_signature_get_the_dummy_one() {
        let msgs = vec![Msg::User("q".into()), Msg::Assistant { text: String::new(), calls: vec![call(None)] }];
        let out = gemini_contents(&ChatRequest { system: "s", messages: &msgs, tools: &[] });
        assert_eq!(out[1]["parts"][0]["thoughtSignature"], DUMMY_SIGNATURE);
    }

    #[test]
    fn openai_compat_gemini_signature_round_trips() {
        let mut c = OpenAiCalls::default();
        c.feed(&json!({ "tool_calls": [{ "index": 0, "id": "c1", "function": { "name": "read_page", "arguments": "{}" }, "extra_content": { "google": { "thought_signature": "SIG9" } } }] }));
        let calls = c.finish();
        assert_eq!(calls[0].signature.as_deref(), Some("SIG9"));
        let msgs = vec![Msg::Assistant { text: String::new(), calls }];
        let req = ChatRequest { system: "s", messages: &msgs, tools: &[] };
        assert_eq!(openai_messages(&req, true)[1]["tool_calls"][0]["extra_content"]["google"]["thought_signature"], "SIG9");
    }

    #[test]
    fn openai_compat_dummy_only_for_gemini_endpoints() {
        let msgs = vec![Msg::Assistant { text: String::new(), calls: vec![call(None)] }];
        let req = ChatRequest { system: "s", messages: &msgs, tools: &[] };
        assert_eq!(openai_messages(&req, true)[1]["tool_calls"][0]["extra_content"]["google"]["thought_signature"], DUMMY_SIGNATURE);
        assert!(openai_messages(&req, false)[1]["tool_calls"][0].get("extra_content").is_none());
        assert!(is_gemini_url("https://generativelanguage.googleapis.com/v1beta/openai"));
        assert!(!is_gemini_url("https://api.openai.com/v1"));
    }

    #[test]
    fn anthropic_groups_tool_results_in_one_user_message() {
        let msgs = vec![
            Msg::User("hi".into()),
            Msg::Assistant { text: String::new(), calls: vec![ToolCall { id: "a".into(), name: "n".into(), args: json!({}), signature: None }, ToolCall { id: "b".into(), name: "n".into(), args: json!({}), signature: None }] },
            Msg::ToolResult { call_id: "a".into(), name: "n".into(), content: "1".into() },
            Msg::ToolResult { call_id: "b".into(), name: "n".into(), content: "2".into() },
        ];
        let req = ChatRequest { system: "s", messages: &msgs, tools: &[] };
        let out = anthropic_messages(&req);
        assert_eq!(out.len(), 3);
        assert_eq!(out[2]["content"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn defaults_and_tool_support() {
        assert!(default_base_url("openai_compatible").is_none());
        assert!(!supports_tools("perplexity"));
        assert!(build(&Conn { provider: "openai_compatible".into(), model: "m".into(), base_url: None, api_key: None, quiet: false }).is_err());
    }
}
