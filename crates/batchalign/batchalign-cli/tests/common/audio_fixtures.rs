use std::path::{Path, PathBuf};

/// Resolved paths to audio test fixtures copied into a session temp directory.
pub struct AudioFixtures {
    /// Path to the test MP3 audio file.
    pub audio: PathBuf,
    /// Path to the fully-annotated test CHAT file (with %mor/%gra/%wor).
    pub chat: PathBuf,
    /// Path to a stripped CHAT file (main tiers + headers only, no %mor/%gra/%wor).
    pub stripped_chat: PathBuf,
}

/// Locate the committed audio fixtures and copy them into `session_dir`.
///
/// Returns `None` if source fixtures are missing (tests should skip).
pub fn prepare_audio_fixtures(session_dir: &Path) -> Option<AudioFixtures> {
    let repo_root = super::find_repo_root()?;
    let source_mp3 = repo_root.join("python/batchalign/tests/support/test.mp3");
    let source_cha = repo_root.join("batchalign/tests/formats/chat/support/test.cha");

    if !source_mp3.exists() || !source_cha.exists() {
        eprintln!(
            "SKIP: audio fixtures not found (expected {}, {})",
            source_mp3.display(),
            source_cha.display()
        );
        return None;
    }

    let dest_cha = session_dir.join("test.cha");
    std::fs::copy(&source_cha, &dest_cha).expect("copy test.cha");

    let align_dir = session_dir.join("align_input");
    std::fs::create_dir_all(&align_dir).expect("mkdir align_input");
    let dest_mp3 = align_dir.join("test.mp3");
    let dest_stripped = align_dir.join("test.cha");

    std::fs::copy(&source_mp3, &dest_mp3).expect("copy test.mp3");
    let stripped =
        strip_dependent_tiers(&std::fs::read_to_string(&source_cha).expect("read test.cha"));
    std::fs::write(&dest_stripped, &stripped).expect("write stripped test.cha");

    let transcribe_mp3 = session_dir.join("test.mp3");
    std::fs::copy(&source_mp3, &transcribe_mp3).expect("copy test.mp3 for transcribe");

    Some(AudioFixtures {
        audio: transcribe_mp3,
        chat: dest_cha,
        stripped_chat: dest_stripped,
    })
}

/// Strip %mor, %gra, and %wor dependent tiers from CHAT text.
pub fn strip_dependent_tiers(chat: &str) -> String {
    let mut result = String::new();
    for line in chat.lines() {
        if line.starts_with("%mor:") || line.starts_with("%gra:") || line.starts_with("%wor:") {
            continue;
        }
        result.push_str(line);
        result.push('\n');
    }
    result
}

/// Prepare multi-speaker audio fixtures (eng_multi_speaker.mp3).
pub fn prepare_multi_speaker_audio(session_dir: &Path) -> Option<AudioFixtures> {
    let repo_root = super::find_repo_root()?;
    let source_mp3 = repo_root.join("python/batchalign/tests/support/eng_multi_speaker.mp3");
    let source_cha = repo_root.join("python/batchalign/tests/support/parity/eng_multi_speaker.cha");

    if !source_mp3.exists() || !source_cha.exists() {
        eprintln!(
            "SKIP: multi-speaker fixtures not found ({}, {})",
            source_mp3.display(),
            source_cha.display()
        );
        return None;
    }

    let dest_mp3 = session_dir.join("eng_multi_speaker.mp3");
    let dest_cha = session_dir.join("eng_multi_speaker.cha");
    let dest_stripped = session_dir.join("eng_multi_speaker_stripped.cha");

    std::fs::copy(&source_mp3, &dest_mp3).expect("copy eng_multi_speaker.mp3");
    std::fs::copy(&source_cha, &dest_cha).expect("copy eng_multi_speaker.cha");

    let stripped = strip_dependent_tiers(
        &std::fs::read_to_string(&source_cha).expect("read eng_multi_speaker.cha"),
    );
    std::fs::write(&dest_stripped, &stripped).expect("write stripped eng_multi_speaker.cha");

    Some(AudioFixtures {
        audio: dest_mp3,
        chat: dest_cha,
        stripped_chat: dest_stripped,
    })
}

/// Prepare a named audio clip from `python/batchalign/tests/support/{name}.mp3`.
pub fn prepare_named_audio(
    session_dir: &Path,
    audio_name: &str,
    chat_name: Option<&str>,
) -> Option<AudioFixtures> {
    let repo_root = super::find_repo_root()?;
    let source_mp3 = repo_root.join(format!("python/batchalign/tests/support/{audio_name}.mp3"));

    if !source_mp3.exists() {
        eprintln!("SKIP: audio fixture not found: {}", source_mp3.display());
        return None;
    }

    let transcribe_mp3 = session_dir.join(format!("{audio_name}.mp3"));
    std::fs::copy(&source_mp3, &transcribe_mp3).expect("copy audio for transcribe");

    let (dest_cha, dest_stripped) = if let Some(cn) = chat_name {
        let source_cha = repo_root.join(format!("python/batchalign/tests/support/parity/{cn}.cha"));
        if !source_cha.exists() {
            eprintln!("SKIP: CHAT fixture not found: {}", source_cha.display());
            return None;
        }

        let chat_content = std::fs::read_to_string(&source_cha).expect("read chat");
        let media_basename = chat_content
            .lines()
            .find(|l| l.starts_with("@Media:"))
            .and_then(|l| l.split('\t').nth(1))
            .and_then(|m| m.split(',').next())
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| audio_name.to_string());

        let align_dir = session_dir.join(format!("align_{audio_name}"));
        std::fs::create_dir_all(&align_dir).expect("mkdir align dir");

        let align_mp3 = align_dir.join(format!("{media_basename}.mp3"));
        std::fs::copy(&source_mp3, &align_mp3).expect("copy audio for align");

        let dest = align_dir.join(format!("{media_basename}.cha"));
        std::fs::copy(&source_cha, &dest).expect("copy chat");

        let stripped = strip_dependent_tiers(&chat_content);
        let dest_s = align_dir.join(format!("{media_basename}_input.cha"));
        std::fs::write(&dest_s, &stripped).expect("write stripped chat");

        let dest_align_input = align_dir.join(format!("{media_basename}.cha"));
        std::fs::write(&dest_align_input, &stripped).expect("write align input");

        (dest, dest_align_input)
    } else {
        let dummy = session_dir.join("dummy.cha");
        (dummy.clone(), dummy)
    };

    Some(AudioFixtures {
        audio: transcribe_mp3,
        chat: dest_cha,
        stripped_chat: dest_stripped,
    })
}
