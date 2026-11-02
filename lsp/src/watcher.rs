use std::path::PathBuf;

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc::{self, UnboundedReceiver};

pub struct WatcherHandle {
    _watcher: RecommendedWatcher,
}

pub fn watch(
    paths: Vec<PathBuf>,
) -> notify::Result<(WatcherHandle, UnboundedReceiver<Vec<PathBuf>>)> {
    let (sender, receiver) = mpsc::unbounded_channel();
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<Event>| {
        if let Ok(event) = event {
            if !event.paths.is_empty() {
                let _ = sender.send(event.paths);
            }
        }
    })?;

    for path in paths {
        watcher.watch(&path, RecursiveMode::Recursive)?;
    }

    Ok((WatcherHandle { _watcher: watcher }, receiver))
}
