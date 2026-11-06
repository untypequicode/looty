//! Styles de diagramme d'un espace : le style par défaut que suivent les diagrammes
//! (Mermaid) et les blocs graphe de ses pages, et les styles personnalisés créés par
//! ses administrateurs. Les styles fournis par Looty vivent dans l'interface.
//!
//! Les couleurs finissent dans le CSS des diagrammes : elles sont validées strictement
//! (`#rrggbb`), comme le reste du format, qui n'accepte aucun champ inconnu.

use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::access::{require_ws_admin, AuthUser};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

const MAX_STYLES: usize = 30;

#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DiagramStyles {
    /// Style par défaut de l'espace (fourni par Looty ou personnalisé) ; absent = celui de Looty.
    #[serde(default)]
    pub default: Option<String>,
    #[serde(default)]
    pub styles: Vec<DiagramStyle>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DiagramStyle {
    pub id: String,
    pub name: String,
    pub look: Look,
    pub layout: Layout,
    pub font: Font,
    pub shape: Shape,
    pub light: Palette,
    pub dark: Palette,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Look {
    Classic,
    HandDrawn,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Layout {
    Elk,
    Dagre,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Font {
    Sans,
    Serif,
    Mono,
    Hand,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Shape {
    Rounded,
    Rect,
    Circle,
}

/// Couleurs d'un style, pour le thème clair ou sombre.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Palette {
    pub background: String,
    pub node: String,
    pub border: String,
    pub text: String,
    pub edge: String,
    pub accent: String,
}

fn is_hex(c: &str) -> bool {
    c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|x| x.is_ascii_hexdigit())
}

fn is_id(id: &str) -> bool {
    (1..=40).contains(&id.len()) && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

impl DiagramStyles {
    /// Contenu enregistré (colonne JSONB) ; un contenu illisible vaut « aucun réglage ».
    pub fn parse(raw: &str) -> Self {
        serde_json::from_str(raw).unwrap_or_default()
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        if self.styles.len() > MAX_STYLES {
            return Err("too_many_styles");
        }
        let mut ids = std::collections::HashSet::new();
        for s in &self.styles {
            if !is_id(&s.id) || !ids.insert(s.id.as_str()) {
                return Err("invalid_style_id");
            }
            let name = s.name.trim();
            if name.is_empty() || name.chars().count() > 60 {
                return Err("invalid_style_name");
            }
            for p in [&s.light, &s.dark] {
                if ![&p.background, &p.node, &p.border, &p.text, &p.edge, &p.accent].iter().all(|c| is_hex(c)) {
                    return Err("invalid_value");
                }
            }
        }
        // Le défaut peut être un style fourni par Looty (connu de l'interface seule).
        if self.default.as_deref().is_some_and(|d| !is_id(d)) {
            return Err("invalid_style_id");
        }
        Ok(())
    }
}

pub async fn load(pool: &PgPool, workspace_id: Uuid) -> ApiResult<DiagramStyles> {
    let raw: String = sqlx::query_scalar("SELECT diagram_styles::text FROM workspaces WHERE id = $1")
        .bind(workspace_id)
        .fetch_one(pool)
        .await?;
    Ok(DiagramStyles::parse(&raw))
}

pub async fn get_styles(State(pool): State<PgPool>, user: AuthUser, Path(workspace_id): Path<Uuid>) -> ApiResult<Json<DiagramStyles>> {
    crate::access::require_member_or_guest(&pool, &user, workspace_id).await?;
    Ok(Json(load(&pool, workspace_id).await?))
}

pub async fn put_styles(
    State(state): State<AppState>,
    user: AuthUser,
    Path(workspace_id): Path<Uuid>,
    Json(mut p): Json<DiagramStyles>,
) -> ApiResult<Json<DiagramStyles>> {
    require_ws_admin(&state.pool, &user, workspace_id).await?;
    for s in &mut p.styles {
        s.name = s.name.trim().to_string();
    }
    p.validate().map_err(ApiError::bad_request)?;
    let json = serde_json::to_string(&p).map_err(|_| ApiError::bad_request("invalid_value"))?;
    sqlx::query("UPDATE workspaces SET diagram_styles = $1::jsonb, updated_at = NOW() WHERE id = $2")
        .bind(&json)
        .bind(workspace_id)
        .execute(&state.pool)
        .await?;
    state
        .emit(workspace_id, serde_json::json!({ "type": "WORKSPACE_DIAGRAM_STYLES_UPDATED", "styles": &p }))
        .await;
    Ok(Json(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn palette() -> serde_json::Value {
        serde_json::json!({ "background": "#ffffff", "node": "#eef2ff", "border": "#5e6ad2", "text": "#1f2328", "edge": "#57606a", "accent": "#f5f7ff" })
    }

    fn style(id: &str) -> serde_json::Value {
        serde_json::json!({ "id": id, "name": "Cours", "look": "handDrawn", "layout": "elk", "font": "serif", "shape": "circle", "light": palette(), "dark": palette() })
    }

    #[test]
    fn accepts_a_valid_style() {
        let s: DiagramStyles = serde_json::from_value(serde_json::json!({ "default": "cours", "styles": [style("cours")] })).unwrap();
        assert_eq!(s.validate(), Ok(()));
        assert_eq!(s.styles[0].look, Look::HandDrawn);
    }

    #[test]
    fn empty_column_means_no_settings() {
        assert_eq!(DiagramStyles::parse("{}"), DiagramStyles::default());
        assert_eq!(DiagramStyles::parse("not json"), DiagramStyles::default());
    }

    #[test]
    fn rejects_colors_that_are_not_hex() {
        let mut v = style("cours");
        v["light"]["node"] = "red;}body{display:none".into();
        let s: DiagramStyles = serde_json::from_value(serde_json::json!({ "styles": [v] })).unwrap();
        assert_eq!(s.validate(), Err("invalid_value"));
    }

    #[test]
    fn rejects_unknown_fields_and_duplicates() {
        let mut v = style("cours");
        v["extra"] = 1.into();
        assert!(serde_json::from_value::<DiagramStyles>(serde_json::json!({ "styles": [v] })).is_err());
        let s: DiagramStyles = serde_json::from_value(serde_json::json!({ "styles": [style("a"), style("a")] })).unwrap();
        assert_eq!(s.validate(), Err("invalid_style_id"));
        let s: DiagramStyles = serde_json::from_value(serde_json::json!({ "default": "Bad Id" })).unwrap();
        assert_eq!(s.validate(), Err("invalid_style_id"));
    }
}
