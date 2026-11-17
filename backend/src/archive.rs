use sqlx::PgPool;
use std::time::Duration;
use tokio::time;
use std::fs;
use flate2::write::GzEncoder;
use flate2::Compression;
use chrono::{Utc, Duration as ChronoDuration};

pub fn start_archive_cron(pool: PgPool) {
    tokio::spawn(async move {
        // Run every 24 hours
        let mut interval = time::interval(Duration::from_secs(24 * 3600));
        
        loop {
            interval.tick().await;
            
            tracing::info!("archiving old task activities");
            
            if let Err(e) = archive_old_activities(&pool).await {
                tracing::warn!("archiving activities: {e}");
            }
            crate::revisions::prune(&pool).await;
            crate::analytics::prune(&pool).await;
        }
    });
}

async fn archive_old_activities(pool: &PgPool) -> Result<(), Box<dyn std::error::Error>> {
    let cutoff_date = Utc::now() - ChronoDuration::days(30);
    
    // Vérifie s'il y a des activités à archiver avant d'allouer quoi que ce soit
    let has_any: Option<uuid::Uuid> = sqlx::query_scalar("SELECT id FROM task_activities WHERE created_at < $1 LIMIT 1")
        .bind(cutoff_date)
        .fetch_optional(pool)
        .await?;
        
    if has_any.is_none() {
        return Ok(());
    }

    #[derive(sqlx::FromRow)]
    struct ActivityRow {
        id: uuid::Uuid,
        task_id: uuid::Uuid,
        user_id: uuid::Uuid,
        activity_type: String,
        old_value: Option<serde_json::Value>,
        new_value: Option<serde_json::Value>,
        created_at: chrono::DateTime<chrono::Utc>,
    }

    let _ = tokio::fs::create_dir_all("uploads/.archives").await;
    let filename = format!("uploads/.archives/activities_{}.json.gz", Utc::now().timestamp());

    // Au plus 2 500 lignes par cycle (RAM bornée) ; suppression seulement après écriture du fichier.
    let batch = sqlx::query_as::<_, ActivityRow>(
        r#"
        SELECT id, task_id, user_id, activity_type, old_value, new_value, created_at
        FROM task_activities
        WHERE created_at < $1
        ORDER BY created_at ASC
        LIMIT 2500
        "#
    )
    .bind(cutoff_date)
    .fetch_all(pool)
    .await?;

    let total_archived = batch.len();
    let ids: Vec<uuid::Uuid> = batch.iter().map(|r| r.id).collect();
    let flat_records: Vec<serde_json::Value> = batch
        .into_iter()
        .map(|row| {
            serde_json::json!({
                "id": row.id,
                "task_id": row.task_id,
                "user_id": row.user_id,
                "activity_type": row.activity_type,
                "old_value": row.old_value,
                "new_value": row.new_value,
                "created_at": row.created_at
            })
        })
        .collect();
    let fname = filename.clone();

    tokio::task::spawn_blocking(move || -> Result<(), std::io::Error> {
        let file = fs::File::create(&fname)?;
        let mut encoder = GzEncoder::new(file, Compression::default());
        serde_json::to_writer(&mut encoder, &flat_records).map_err(std::io::Error::other)?;
        encoder.finish()?;
        Ok(())
    })
    .await
    .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?
    .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    sqlx::query("DELETE FROM task_activities WHERE id = ANY($1)")
        .bind(&ids)
        .execute(pool)
        .await?;

    tracing::info!("Archived {total_archived} activities to {filename}");
    Ok(())
}
