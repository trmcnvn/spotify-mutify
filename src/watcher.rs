use crossbeam_channel::Sender;
use directories::BaseDirs;
use failure::{format_err, Error};
use notify::{RecommendedWatcher, RecursiveMode, Watcher as NotifyWatcher};
use std::fs;
use std::path::{Path, PathBuf};

pub struct Watcher;
impl Watcher {
    pub fn watch(
        sender: Sender<notify::Result<notify::Event>>,
    ) -> Result<RecommendedWatcher, Error> {
        // Create a watcher with a 500ms delay
        let mut watcher = notify::watcher(sender, std::time::Duration::from_millis(500))?;

        // Watch the target directory
        let target_path = Watcher::find_data_directory()?;
        for entry in fs::read_dir(&target_path)? {
            let dir = entry?;
            let dir_str =
                dir.path().into_os_string().into_string().map_err(|err| {
                    format_err!("Couldn't convert PathBuf into String: {:?}", err)
                })?;
            if dir_str.contains("-user") {
                watcher.watch(dir.path(), RecursiveMode::NonRecursive)?;
            }
        }
        Ok(watcher)
    }

    pub fn is_target_event(event: notify::Event) -> bool {
        event.paths.iter().any(|x| {
            if let Some(file_name) = x.file_name() {
                return file_name == "ad-state-storage.bnk" || file_name == "recently_played.bnk";
            }
            false
        })
    }

    fn find_data_directory() -> Result<PathBuf, Error> {
        let base_dirs =
            BaseDirs::new().ok_or_else(|| format_err!("Couldn't get directory information"))?;

        // Search in the usual data directory
        let mut target_path = base_dirs.data_dir().join("Spotify\\Users");
        if Path::new(&target_path).exists() {
            return Ok(target_path);
        }

        // Could be a Windows Store install which has a different location
        if cfg!(windows) {
            target_path = base_dirs.data_local_dir().join("Packages");
            if Path::new(&target_path).exists() {
                // Iterate to find the Spotify folder
                for entry in fs::read_dir(&target_path)? {
                    let dir = entry?;
                    let dir_str = dir.path().into_os_string().into_string().map_err(|err| {
                        format_err!("Couldn't convert PathBuf into String: {:?}", err)
                    })?;
                    if dir_str.contains("SpotifyAB.SpotifyMusic") {
                        target_path =
                            target_path.join(format!("{}\\LocalState\\Spotify\\Users", dir_str));
                        if Path::new(&target_path).exists() {
                            return Ok(target_path);
                        }
                    }
                }
            }
        }

        // not installed?
        Err(format_err!("Couldn't find the Spotify data directory."))
    }
}
