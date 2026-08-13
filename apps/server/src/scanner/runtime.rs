use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::atomic::Ordering,
    time::Duration,
};

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::{error::AppError, library::LibraryRecord, state::AppState};

use super::enqueue_scan;

pub fn start_background_services(state: AppState) {
    refresh_watchers(state.clone());
    tokio::spawn(async move {
        loop {
            let interval = state.runtime.read().await.editable.reconcile_interval_sec;
            tokio::time::sleep(Duration::from_secs(interval)).await;
            if let Err(error) = enqueue_scheduled_scans(state.clone()).await {
                tracing::error!(%error, "创建定时扫描任务失败");
            }
        }
    });
}

pub fn refresh_watchers(state: AppState) {
    let generation = state.watch_generation.fetch_add(1, Ordering::SeqCst) + 1;
    tokio::spawn(async move {
        if let Err(error) = watch_libraries(state, generation).await {
            tracing::warn!(%error, "Library Watcher 已停止");
        }
    });
}

async fn enqueue_scheduled_scans(state: AppState) -> Result<(), AppError> {
    let ids = sqlx::query_scalar::<_, Uuid>("SELECT id FROM libraries WHERE scan_enabled = 1")
        .fetch_all(&state.pool)
        .await
        .map_err(AppError::internal)?;
    for id in ids {
        let _ = enqueue_scan(state.clone(), id).await?;
    }
    Ok(())
}

async fn watch_libraries(state: AppState, generation: u64) -> anyhow::Result<()> {
    let libraries = sqlx::query_as::<_, LibraryRecord>(
        r#"
        SELECT id, name, path, scan_enabled, watch_enabled, writable, role,
               target_library_id, auto_ingest_enabled, exclude_patterns,
               last_scan_at, created_at, updated_at
        FROM libraries WHERE scan_enabled = 1 AND watch_enabled = 1
        "#,
    )
    .fetch_all(&state.pool)
    .await?;
    if libraries.is_empty() {
        return Ok(());
    }
    let (sender, mut receiver) = mpsc::unbounded_channel();
    let mut watcher: RecommendedWatcher = notify::recommended_watcher(move |event| {
        let _ = sender.send(event);
    })?;
    for library in &libraries {
        watcher.watch(Path::new(&library.path), RecursiveMode::Recursive)?;
    }

    loop {
        tokio::select! {
            event = receiver.recv() => {
                let Some(Ok(event)) = event else { continue };
                let mut affected = affected_libraries(&libraries, &event.paths);
                let debounce = state.runtime.read().await.editable.watch_debounce_sec;
                tokio::time::sleep(Duration::from_secs(debounce)).await;
                while let Ok(Ok(next_event)) = receiver.try_recv() {
                    affected.extend(affected_libraries(&libraries, &next_event.paths));
                }
                for id in affected {
                    if let Err(error) = enqueue_scan(state.clone(), id).await {
                        tracing::warn!(library_id = %id, %error, "Watcher 创建扫描任务失败");
                    }
                }
            }
            _ = tokio::time::sleep(Duration::from_secs(2)) => {
                if state.watch_generation.load(Ordering::SeqCst) != generation {
                    break;
                }
            }
        }
    }
    Ok(())
}

fn affected_libraries(libraries: &[LibraryRecord], paths: &[PathBuf]) -> HashSet<Uuid> {
    libraries
        .iter()
        .filter(|library| paths.iter().any(|path| path.starts_with(&library.path)))
        .map(|library| library.id)
        .collect()
}
