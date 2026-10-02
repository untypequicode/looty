use sqlx::PgPool;
use std::time::Duration;
use tokio::time;
use std::fs;
use std::io::Write;
use flate2::write::GzEncoder;
use flate2::Compression;
use chrono::{Utc, Duration as ChronoDuration};

pub fn start_archive_cron(pool: PgPool) {
    tokio::spawn(async move {
        // Run every 24 hours
        let mut interval = time::interval(Duration::from_secs(24 * 3600));
        
        loop {
            interval.tick().await;
            
            println!("Running automated archiving for task_activities...");
            
            if let Err(e) = archive_old_activities(&pool).await {
                eprintln!("Error archiving activities: {}", e);
            }
        }
    });
}

async fn archive_old_activities(pool: &PgPool) -> Result<(), Box<dyn std::error::Error>> {
    let cutoff_date = Utc::now() - ChronoDuration::days(30);
    
    // Fetch activities older than 30 days
    // Note: We need a struct to deserialize them
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

    let old_activities = sqlx::query_as::<_, ActivityRow>(
        r#"
        SELECT id, task_id, user_id, activity_type, old_value, new_value, created_at
        FROM task_activities
        WHERE created_at < $1
        "#
    )
    .bind(cutoff_date)
    .fetch_all(pool)
    .await?;
    
    if old_activities.is_empty() {
        return Ok(());
    }
    
    // Convert to JSON
    let mut json_records = Vec::new();
    for row in &old_activities {
        let record = serde_json::json!({
            "id": row.id,
            "task_id": row.task_id,
            "user_id": row.user_id,
            "activity_type": row.activity_type,
            "old_value": row.old_value,
            "new_value": row.new_value,
            "created_at": row.created_at
        });
        json_records.push(record);
    }
    
    let json_data = serde_json::to_string(&json_records)?;
    
    // Ensure archive directory exists
    let _ = fs::create_dir_all("uploads/.archives");
    
    // Compress and save
    let filename = format!("uploads/.archives/activities_{}.json.gz", Utc::now().timestamp());
    let file = fs::File::create(&filename)?;
    let mut encoder = GzEncoder::new(file, Compression::default());
    encoder.write_all(json_data.as_bytes())?;
    encoder.finish()?;
    
    println!("Archived {} activities to {}", old_activities.len(), filename);
    
    // Delete from DB
    sqlx::query("DELETE FROM task_activities WHERE created_at < $1")
        .bind(cutoff_date)
        .execute(pool)
    .await?;
    
    Ok(())
}
