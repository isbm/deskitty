//! Embedded action sounds.
//!
//! The WAV files under `sounds/` are compiled into the binary.

/// One embedded sound.
#[derive(Debug, Clone, Copy)]
pub struct Sound {
    /// Name used in the configuration (`sound: accept`).
    pub name: &'static str,
    /// Raw WAV bytes.
    pub bytes: &'static [u8],
}

/// All sounds available to action rules.
pub static SOUNDS: &[Sound] = &[
    Sound { name: "accept", bytes: include_bytes!("../sounds/accept.wav") },
    Sound { name: "tada", bytes: include_bytes!("../sounds/tada.wav") },
    Sound { name: "wxp", bytes: include_bytes!("../sounds/wxp.wav") },
];

/// Find an embedded sound by name.
pub fn find(name: &str) -> Option<&'static Sound> {
    SOUNDS.iter().find(|sound| sound.name == name)
}

/// Iterate over the available sound names.
pub fn names() -> impl Iterator<Item = &'static str> {
    SOUNDS.iter().map(|sound| sound.name)
}
