//! Command line values: instance URL, token, whitelist of local services.

use crate::proto::ServiceDecl;
use std::collections::HashMap;
use url::Url;

#[derive(Clone)]
pub struct Service {
    pub decl: ServiceDecl,
    /// Local base URL, known only to this process.
    pub base: String,
}

/// The whitelist: the only places the agent will ever connect to.
#[derive(Clone, Default)]
pub struct Services(pub HashMap<String, Service>);

impl Services {
    pub fn parse(specs: &[String]) -> Result<Services, String> {
        let mut m = HashMap::new();
        for s in specs {
            let svc = parse_service(s)?;
            if m.contains_key(&svc.decl.name) {
                return Err(format!("service '{}' declared twice", svc.decl.name));
            }
            m.insert(svc.decl.name.clone(), svc);
        }
        if m.len() > 16 {
            return Err("at most 16 services".into());
        }
        Ok(Services(m))
    }

    /// Local base URL of a declared service, `None` for anything else.
    pub fn base(&self, name: &str) -> Option<&str> {
        self.0.get(name).map(|s| s.base.as_str())
    }

    pub fn decls(&self) -> Vec<ServiceDecl> {
        let mut v: Vec<_> = self.0.values().map(|s| s.decl.clone()).collect();
        v.sort_by(|a, b| a.name.cmp(&b.name));
        v
    }
}

/// `name=url` or `name:kind=url`. Kind defaults from the name (`jupyter`, `latex`/`tex`, else `http`).
pub fn parse_service(spec: &str) -> Result<Service, String> {
    let (lhs, url) = spec
        .split_once('=')
        .ok_or_else(|| format!("service '{spec}': expected name=http://host:port"))?;
    let (name, kind) = match lhs.split_once(':') {
        Some((n, k)) => (n, k.to_string()),
        None => (
            lhs,
            match lhs {
                "jupyter" => "jupyter",
                "latex" | "tex" => "latex",
                _ => "http",
            }
            .to_string(),
        ),
    };
    let name_ok = !name.is_empty()
        && name.len() <= 32
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
    if !name_ok {
        return Err(format!(
            "service name '{name}': 1-32 characters among a-z 0-9 - _"
        ));
    }
    if !["jupyter", "latex", "http"].contains(&kind.as_str()) {
        return Err(format!(
            "service '{name}': kind must be jupyter, latex or http"
        ));
    }
    let u = Url::parse(url).map_err(|e| format!("service '{name}': bad URL: {e}"))?;
    if u.scheme() != "http" {
        return Err(format!(
            "service '{name}': only http:// local addresses are supported"
        ));
    }
    if u.host_str().is_none() || u.query().is_some() || u.fragment().is_some() {
        return Err(format!("service '{name}': URL needs a host and no query"));
    }
    Ok(Service {
        decl: ServiceDecl {
            name: name.into(),
            kind,
        },
        base: u.as_str().trim_end_matches('/').to_string(),
    })
}

/// Maps the instance URL to the agent WebSocket URL. Plain `http://` is only
/// accepted for a loopback host (local tests).
pub fn agent_url(instance: &str) -> Result<String, String> {
    let u = Url::parse(instance).map_err(|e| format!("bad --instance: {e}"))?;
    let host = u.host_str().ok_or("--instance has no host")?;
    let loopback = host == "localhost" || host == "127.0.0.1" || host == "[::1]";
    let scheme = match u.scheme() {
        "https" => "wss",
        "http" if loopback => "ws",
        "http" => return Err("refusing plain http:// to a remote instance; use https://".into()),
        _ => return Err("--instance must start with https://".into()),
    };
    let mut out = u.clone();
    out.set_scheme(scheme).map_err(|_| "bad scheme")?;
    out.set_query(None);
    out.set_fragment(None);
    let base = out.as_str().trim_end_matches('/');
    Ok(format!("{base}/api/tunnels/agent"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitelist_only_declared() {
        let s = Services::parse(&[
            "jupyter=http://127.0.0.1:8888/".into(),
            "tex:latex=http://tex:80".into(),
        ])
        .unwrap();
        assert_eq!(s.base("jupyter"), Some("http://127.0.0.1:8888"));
        assert_eq!(s.base("tex"), Some("http://tex"));
        assert_eq!(s.base("other"), None);
        assert_eq!(s.base("127.0.0.1"), None);
        let d = s.decls();
        assert_eq!(d[0].kind, "jupyter");
        assert_eq!(d[1].kind, "latex");
    }

    #[test]
    fn rejects_bad_services() {
        for bad in [
            "x",
            "A=http://a",
            "a=ftp://a",
            "a:zzz=http://a",
            "a=http://a?x=1",
            "=http://a",
        ] {
            assert!(Services::parse(&[bad.into()]).is_err(), "{bad}");
        }
        assert!(Services::parse(&["a=http://a".into(), "a=http://b".into()]).is_err());
    }

    #[test]
    fn instance_urls() {
        assert_eq!(
            agent_url("https://looty.example.org/").unwrap(),
            "wss://looty.example.org/api/tunnels/agent"
        );
        assert_eq!(
            agent_url("http://127.0.0.1:3000").unwrap(),
            "ws://127.0.0.1:3000/api/tunnels/agent"
        );
        assert!(agent_url("http://looty.example.org").is_err());
        assert!(agent_url("ftp://x").is_err());
    }
}
