use crate::sounds;

#[test]
fn knows_embedded_sounds() {
    for name in ["accept", "tada", "wxp"] {
        assert!(sounds::find(name).is_some(), "missing sound: {name}");
    }
    assert!(sounds::find("missing").is_none());
}

#[test]
fn embedded_sounds_have_wav_data() {
    for sound in sounds::SOUNDS {
        assert!(sound.bytes.starts_with(b"RIFF"), "{} is not a WAV file", sound.name);
    }
}
