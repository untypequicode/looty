//! Paramètres de requête booléens tolérants (`?download=1`, `?download`, `?download=True`…).
//!
//! `serde` n'accepte que `true`/`false` pour un `bool` : un lien `?download=1` (écrit par un client,
//! une WebView, un script) faisait échouer toute la requête avec le texte brut de serde.

use axum::{
    extract::{FromRequestParts, Query},
    http::request::Parts,
};
use serde::{de::DeserializeOwned, Deserialize, Deserializer};

use crate::error::ApiError;

/// `true`/`1`/`yes`/`on`/vide, `false`/`0`/`no`/`off` (casse et espaces ignorés).
pub fn parse_flag(s: &str) -> Option<bool> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "true" | "1" | "yes" | "on" => Some(true),
        "false" | "0" | "no" | "off" => Some(false),
        _ => None,
    }
}

/// À utiliser avec `#[serde(default, deserialize_with = "crate::flag::flag")]` sur un `bool`.
pub fn flag<'de, D: Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    let s = String::deserialize(d)?;
    parse_flag(&s).ok_or_else(|| serde::de::Error::custom("invalid boolean"))
}

/// Comme [`flag`], pour un `Option<bool>` (absent = `None`).
pub fn opt_flag<'de, D: Deserializer<'de>>(d: D) -> Result<Option<bool>, D::Error> {
    flag(d).map(Some)
}

/// `Query<T>` dont l'échec est un 400 JSON clair plutôt que le texte brut de serde.
pub struct Q<T>(pub T);

impl<S: Send + Sync, T: DeserializeOwned> FromRequestParts<S> for Q<T> {
    type Rejection = ApiError;
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, ApiError> {
        Query::<T>::from_request_parts(parts, state)
            .await
            .map(|Query(v)| Q(v))
            .map_err(|_| ApiError::bad_request("invalid_query"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request, routing::get, Router};
    use tower::ServiceExt;

    #[derive(Deserialize)]
    struct P {
        #[serde(default, deserialize_with = "flag")]
        download: bool,
    }

    async fn h(Q(p): Q<P>) -> String {
        p.download.to_string()
    }

    async fn call(uri: &str) -> (u16, String) {
        let app = Router::new().route("/", get(h));
        let r = app.oneshot(Request::get(uri).body(Body::empty()).unwrap()).await.unwrap();
        let st = r.status().as_u16();
        let b = axum::body::to_bytes(r.into_body(), 4096).await.unwrap();
        (st, String::from_utf8_lossy(&b).into_owned())
    }

    #[tokio::test]
    async fn accepted_spellings() {
        for v in ["true", "True", "TRUE", "1", "yes", "YES", "on", "On", ""] {
            assert_eq!(call(&format!("/?download={v}")).await, (200, "true".into()), "{v:?}");
        }
        for v in ["false", "False", "0", "no", "off", "OFF"] {
            assert_eq!(call(&format!("/?download={v}")).await, (200, "false".into()), "{v:?}");
        }
        assert_eq!(call("/?download").await, (200, "true".into()));
        assert_eq!(call("/").await, (200, "false".into()));
    }

    #[tokio::test]
    async fn garbage_is_a_clean_400() {
        let (st, body) = call("/?download=banana").await;
        assert_eq!(st, 400);
        assert_eq!(body, r#"{"error":"invalid_query"}"#);
        assert!(!body.contains("deserialize"));
    }
}
