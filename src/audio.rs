use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::sounds;

#[derive(Debug, Clone, Copy)]
struct Player {
    command: &'static str,
    args: &'static [&'static str],
}

const PLAYERS: &[Player] = &[
    Player { command: "paplay", args: &[] },
    Player { command: "pw-play", args: &[] },
    Player { command: "aplay", args: &["-q"] },
    Player { command: "ffplay", args: &["-nodisp", "-autoexit", "-loglevel", "quiet"] },
];

/// Plays embedded sounds through a system audio player.
pub struct SoundPlayer {
    player: Option<&'static Player>,
    dir: Option<PathBuf>,
    reason: Option<String>,
}

impl SoundPlayer {
    /// Create a player, preparing temporary WAV files from embedded bytes.
    ///
    /// When no supported player is available, `reason` describes why sounds
    /// are disabled.
    pub fn new() -> Self {
        let Some(player) = PLAYERS.iter().find(|player| command_exists(player.command)) else {
            return Self { player: None, dir: None, reason: Some("no supported audio player was found".to_string()) };
        };

        let dir = std::env::temp_dir().join(format!("deskitty-sounds-{}", std::process::id()));
        if let Err(err) = prepare_sounds(&dir) {
            return Self { player: None, dir: None, reason: Some(format!("embedded sounds could not be prepared: {err}")) };
        }

        Self { player: Some(player), dir: Some(dir), reason: None }
    }

    /// Return why sounds are unavailable, if they are.
    pub fn reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }

    /// Play an embedded sound by name.
    ///
    /// Playback is non-blocking. A missing or failing player is ignored.
    pub fn play(&mut self, name: &str) {
        let (Some(player), Some(dir)) = (self.player, self.dir.as_ref()) else { return };
        let path = dir.join(format!("{name}.wav"));
        if !path.is_file() {
            return;
        }

        let mut command = Command::new(player.command);
        command.args(player.args.iter().copied()).arg(&path).stdout(Stdio::null()).stderr(Stdio::null());
        match command.spawn() {
            Ok(mut child) => {
                std::thread::spawn(move || {
                    let _ = child.wait();
                });
            }
            Err(_) => {
                self.player = None;
                self.reason = Some("the audio player could not be started".to_string());
            }
        }
    }
}

impl Default for SoundPlayer {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for SoundPlayer {
    fn drop(&mut self) {
        if let Some(dir) = &self.dir {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

fn prepare_sounds(dir: &Path) -> std::io::Result<()> {
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir)?;
    for sound in sounds::SOUNDS {
        std::fs::write(dir.join(format!("{}.wav", sound.name)), sound.bytes)?;
    }
    Ok(())
}

fn command_exists(command: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|path| {
        std::env::split_paths(&path).any(|dir| {
            let path = dir.join(command);
            path.is_file() && path.metadata().is_ok_and(|metadata| metadata.permissions().mode() & 0o111 != 0)
        })
    })
}
