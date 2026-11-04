//! Sondages (GLOSSARY) : portés par une annonce. Une question, 2 à 10 options, choix
//! unique ou multiple ; chaque membre vote, change ou retire son vote tant que le sondage
//! est ouvert. Anonyme : seuls les totaux sortent du serveur. « Résultats après le vote » :
//! les totaux restent cachés à qui n'a pas voté, jusqu'à la clôture.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use uuid::Uuid;

use crate::access::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

#[derive(Deserialize, Clone)]
pub struct PollReq {
    pub question: String,
    #[serde(default)]
    pub options: Vec<String>,
    #[serde(default)]
    pub multiple: bool,
    #[serde(default)]
    pub anonymous: bool,
    #[serde(default)]
    pub results_after_vote: bool,
    pub closes_at: Option<DateTime<Utc>>,
}

/// Question et options nettoyées (texte non vide, sans doublon, 2 à 10 options).
pub fn validate(p: &PollReq) -> ApiResult<(String, Vec<String>)> {
    let question = p.question.trim().to_string();
    if question.is_empty() || question.chars().count() > 300 {
        return Err(ApiError::bad_request("poll_invalid_question"));
    }
    let mut options: Vec<String> = Vec::new();
    for o in &p.options {
        let o = o.trim();
        if o.chars().count() > 200 {
            return Err(ApiError::bad_request("poll_invalid_option"));
        }
        if !o.is_empty() && !options.iter().any(|x| x.to_lowercase() == o.to_lowercase()) {
            options.push(o.to_string());
        }
    }
    if !(2..=10).contains(&options.len()) {
        return Err(ApiError::bad_request("poll_option_count"));
    }
    Ok((question, options))
}

/// Crée ou remplace le sondage d'une annonce. Une fois des votes reçus, seules la question
/// et la clôture changent (les options et les règles de vote restent).
pub async fn save(tx: &mut sqlx::PgConnection, announcement: Uuid, p: &PollReq) -> ApiResult<()> {
    let (question, options) = validate(p)?;
    let votes: i64 = sqlx::query_scalar("SELECT count(*) FROM poll_votes WHERE announcement_id = $1").bind(announcement).fetch_one(&mut *tx).await?;
    if votes > 0 {
        sqlx::query("UPDATE announcement_polls SET question = $2, closes_at = $3 WHERE announcement_id = $1")
            .bind(announcement)
            .bind(&question)
            .bind(p.closes_at)
            .execute(&mut *tx)
            .await?;
        return Ok(());
    }
    sqlx::query("DELETE FROM announcement_polls WHERE announcement_id = $1").bind(announcement).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO announcement_polls (announcement_id, question, multiple, anonymous, results_after_vote, closes_at) VALUES ($1, $2, $3, $4, $5, $6)")
        .bind(announcement)
        .bind(&question)
        .bind(p.multiple)
        .bind(p.anonymous)
        .bind(p.results_after_vote)
        .bind(p.closes_at)
        .execute(&mut *tx)
        .await?;
    for (i, o) in options.iter().enumerate() {
        sqlx::query("INSERT INTO poll_options (announcement_id, label, position) VALUES ($1, $2, $3)")
            .bind(announcement)
            .bind(o)
            .bind(i as i32)
            .execute(&mut *tx)
            .await?;
    }
    Ok(())
}

pub async fn delete(tx: &mut sqlx::PgConnection, announcement: Uuid) -> ApiResult<()> {
    sqlx::query("DELETE FROM announcement_polls WHERE announcement_id = $1").bind(announcement).execute(&mut *tx).await?;
    Ok(())
}

#[derive(sqlx::FromRow)]
struct PollRow {
    announcement_id: Uuid,
    question: String,
    multiple: bool,
    anonymous: bool,
    results_after_vote: bool,
    closes_at: Option<DateTime<Utc>>,
    closed: bool,
}

#[derive(sqlx::FromRow)]
struct OptionRow {
    id: Uuid,
    announcement_id: Uuid,
    label: String,
}

#[derive(Serialize)]
struct OptionDto {
    id: Uuid,
    label: String,
    /// Absent tant que les résultats sont cachés.
    votes: Option<i64>,
    /// Qui a choisi cette option (sondage nominatif, résultats visibles).
    voters: Option<Vec<Uuid>>,
}

/// Ce qu'un membre voit d'un sondage, selon ses règles et son vote.
fn view(p: &PollRow, options: &[OptionRow], votes: &[(Uuid, Uuid)], me: Uuid, now: DateTime<Utc>) -> Value {
    let closed = p.closed || p.closes_at.is_some_and(|c| c <= now);
    let mine: Vec<Uuid> = votes.iter().filter(|(_, u)| *u == me).map(|(o, _)| *o).collect();
    let show = closed || !p.results_after_vote || !mine.is_empty();
    let mut voters: Vec<Uuid> = votes.iter().map(|(_, u)| *u).collect();
    voters.sort();
    voters.dedup();
    let opts: Vec<OptionDto> = options
        .iter()
        .map(|o| {
            let on: Vec<Uuid> = votes.iter().filter(|(opt, _)| *opt == o.id).map(|(_, u)| *u).collect();
            OptionDto {
                id: o.id,
                label: o.label.clone(),
                votes: show.then_some(on.len() as i64),
                voters: (show && !p.anonymous).then_some(on),
            }
        })
        .collect();
    json!({
        "question": p.question,
        "multiple": p.multiple,
        "anonymous": p.anonymous,
        "results_after_vote": p.results_after_vote,
        "closes_at": p.closes_at,
        "closed": closed,
        "results_visible": show,
        "voter_count": voters.len(),
        "my_votes": mine,
        "options": opts,
    })
}

/// Sondages des annonces données, tels que `me` les voit.
pub async fn load(pool: &sqlx::PgPool, ids: &[Uuid], me: Uuid) -> ApiResult<HashMap<Uuid, Value>> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let polls: Vec<PollRow> = sqlx::query_as(
        "SELECT announcement_id, question, multiple, anonymous, results_after_vote, closes_at, closed FROM announcement_polls WHERE announcement_id = ANY($1)",
    )
    .bind(ids)
    .fetch_all(pool)
    .await?;
    let options: Vec<OptionRow> = sqlx::query_as("SELECT id, announcement_id, label FROM poll_options WHERE announcement_id = ANY($1) ORDER BY position")
        .bind(ids)
        .fetch_all(pool)
        .await?;
    let votes: Vec<(Uuid, Uuid, Uuid)> = sqlx::query_as("SELECT announcement_id, option_id, user_id FROM poll_votes WHERE announcement_id = ANY($1)")
        .bind(ids)
        .fetch_all(pool)
        .await?;
    let now = Utc::now();
    Ok(polls
        .iter()
        .map(|p| {
            let opts: Vec<OptionRow> = options.iter().filter(|o| o.announcement_id == p.announcement_id).map(|o| OptionRow { id: o.id, announcement_id: o.announcement_id, label: o.label.clone() }).collect();
            let v: Vec<(Uuid, Uuid)> = votes.iter().filter(|(a, _, _)| *a == p.announcement_id).map(|(_, o, u)| (*o, *u)).collect();
            (p.announcement_id, view(p, &opts, &v, me, now))
        })
        .collect())
}

#[derive(Deserialize)]
pub struct VoteReq {
    /// Options choisies ; vide = retirer son vote.
    options: Vec<Uuid>,
}

/// Voter (remplace le vote précédent).
pub async fn vote(State(state): State<AppState>, user: AuthUser, Path((ws, id)): Path<(Uuid, Uuid)>, Json(req): Json<VoteReq>) -> ApiResult<StatusCode> {
    // Seuls ceux qui voient l'annonce (destinataires, auteur, gestionnaires) votent.
    crate::announcements::require_visible(&state, &user, ws, id).await?;
    let poll: Option<(bool, bool, Option<DateTime<Utc>>)> = sqlx::query_as(
        "SELECT p.multiple, p.closed, p.closes_at FROM announcement_polls p JOIN announcements a ON a.id = p.announcement_id
         WHERE p.announcement_id = $1 AND a.workspace_id = $2",
    )
    .bind(id)
    .bind(ws)
    .fetch_optional(&state.pool)
    .await?;
    let (multiple, closed, closes_at) = poll.ok_or(ApiError::NotFound)?;
    if closed || closes_at.is_some_and(|c| c <= Utc::now()) {
        return Err(ApiError::bad_request("poll_closed"));
    }
    let mut chosen = req.options.clone();
    chosen.sort();
    chosen.dedup();
    if !multiple && chosen.len() > 1 {
        return Err(ApiError::bad_request("poll_single_choice"));
    }
    let valid: i64 = sqlx::query_scalar("SELECT count(*) FROM poll_options WHERE announcement_id = $1 AND id = ANY($2)")
        .bind(id)
        .bind(&chosen)
        .fetch_one(&state.pool)
        .await?;
    if valid as usize != chosen.len() {
        return Err(ApiError::bad_request("poll_invalid_option"));
    }
    let mut tx = state.pool.begin().await?;
    sqlx::query("DELETE FROM poll_votes WHERE announcement_id = $1 AND user_id = $2").bind(id).bind(user.id).execute(&mut *tx).await?;
    for o in &chosen {
        sqlx::query("INSERT INTO poll_votes (option_id, announcement_id, user_id) VALUES ($1, $2, $3)").bind(o).bind(id).bind(user.id).execute(&mut *tx).await?;
    }
    // Voter, c'est avoir lu l'annonce.
    sqlx::query("INSERT INTO announcement_reads (announcement_id, user_id) VALUES ($1, $2) ON CONFLICT DO NOTHING").bind(id).bind(user.id).execute(&mut *tx).await?;
    tx.commit().await?;
    state.emit(ws, json!({ "type": "ANNOUNCEMENTS_UPDATED" })).await;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(options: &[&str]) -> PollReq {
        PollReq { question: " Quelle date ? ".into(), options: options.iter().map(|s| s.to_string()).collect(), multiple: false, anonymous: false, results_after_vote: false, closes_at: None }
    }

    #[test]
    fn options_are_cleaned_and_counted() {
        let (q, o) = validate(&req(&["Lundi", " lundi ", "", "Mardi"])).unwrap();
        assert_eq!(q, "Quelle date ?");
        assert_eq!(o, vec!["Lundi", "Mardi"]);
        assert!(validate(&req(&["Seule"])).is_err());
        assert!(validate(&req(&["a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k"])).is_err());
    }

    fn poll(anonymous: bool, after: bool) -> PollRow {
        PollRow { announcement_id: Uuid::nil(), question: "?".into(), multiple: false, anonymous, results_after_vote: after, closes_at: None, closed: false }
    }

    #[test]
    fn results_and_voters_follow_the_rules() {
        let (a, b, me, other) = (Uuid::from_u128(1), Uuid::from_u128(2), Uuid::from_u128(10), Uuid::from_u128(11));
        let opts = vec![OptionRow { id: a, announcement_id: Uuid::nil(), label: "A".into() }, OptionRow { id: b, announcement_id: Uuid::nil(), label: "B".into() }];
        let votes = vec![(a, other)];
        let now = Utc::now();

        let v = view(&poll(false, false), &opts, &votes, me, now);
        assert_eq!(v["options"][0]["votes"], 1);
        assert_eq!(v["options"][0]["voters"][0], other.to_string());
        assert_eq!(v["voter_count"], 1);

        let v = view(&poll(true, false), &opts, &votes, me, now);
        assert_eq!(v["options"][0]["votes"], 1);
        assert!(v["options"][0]["voters"].is_null(), "anonyme : personne n'est nommé");

        let v = view(&poll(false, true), &opts, &votes, me, now);
        assert!(v["options"][0]["votes"].is_null(), "caché avant mon vote");
        let v = view(&poll(false, true), &opts, &[(a, other), (b, me)], me, now);
        assert_eq!(v["options"][1]["votes"], 1);
        assert_eq!(v["my_votes"][0], b.to_string());

        let mut closed = poll(false, true);
        closed.closes_at = Some(now - chrono::Duration::minutes(1));
        let v = view(&closed, &opts, &votes, me, now);
        assert_eq!(v["closed"], true);
        assert_eq!(v["options"][0]["votes"], 1, "visible à la clôture");
    }
}
