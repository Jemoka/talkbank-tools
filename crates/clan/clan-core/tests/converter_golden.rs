//! Golden tests comparing clan-core converter output against CLAN CLI output.
//!
//! These tests run the legacy CLAN converter binaries (C/C++) on fixture files
//! and compare their output against our Rust reimplementation.
//!
//! # Requirements
//!
//! - CLAN binaries must be available at the path specified by `CLAN_BIN_DIR`
//! - Tests are skipped if the binaries are not found (CI-safe)
//!
//! # Snapshot Review
//!
//! Each test produces two insta snapshots:
//! - `<test>@clan` — the legacy CLAN output (reference)
//! - `<test>@rust` — our Rust implementation output
//!
//! Converters without a CLAN binary (lab2chat, rtf2chat, chat2srt) produce
//! only a `@rust` snapshot.
//!
//! Review with `cargo insta review -p clan-core`.

mod common;

use std::path::Path;

use common::{ClanTempDirRun, corpus_file, fixture_file, require_clan_command};
use talkbank_model::{ChatFile, ParseValidateOptions, WriteChat};

/// Run a CLAN to-CHAT converter by piping fixture content via stdin.
///
/// CLAN to-CHAT converters write output to `pipeout.cha` in the working directory.
fn run_clan_to_chat(command: &str, fixture: &Path, args: &[&str]) -> Option<String> {
    let run = ClanTempDirRun::from_stdin(command, fixture, args)?;
    run.read_named_file("pipeout.cha")
        .or_else(|| run.read_first_with_extension("cha"))
}

/// Run a CLAN from-CHAT converter by piping .cha content via stdin.
///
/// Returns the output file content. The output file extension varies by
/// converter (e.g., `.TextGrid` for chat2praat, `.srt` for chat2srt).
fn run_clan_from_chat(command: &str, file: &Path, args: &[&str]) -> Option<String> {
    let run = ClanTempDirRun::from_stdin(command, file, args)?;
    run.read_first_matching_file(|path| {
        path.extension().is_some_and(|ext| ext != "cha")
            || path
                .file_name()
                .is_some_and(|name| name.to_string_lossy().contains("pipeout"))
    })
}

fn load_chat_file(file: &Path) -> ChatFile {
    let content = std::fs::read_to_string(file).expect("Failed to read corpus file");
    talkbank_transform::parse_and_validate(&content, ParseValidateOptions::default())
        .expect("Failed to parse file")
}

fn run_to_chat_case(
    fixture_name: &str,
    clan_command: Option<&str>,
    clan_args: &[&str],
    rust_snapshot: &str,
    clan_snapshot: Option<&str>,
    rust_output: impl FnOnce(&str) -> String,
) {
    let fixture = fixture_file(fixture_name);
    let content = std::fs::read_to_string(&fixture).expect("Failed to read fixture");
    let rust_output = rust_output(&content);
    insta::assert_snapshot!(rust_snapshot, &rust_output);

    let (Some(clan_command), Some(clan_snapshot)) = (clan_command, clan_snapshot) else {
        return;
    };

    if !require_clan_command(clan_command, "skipping CLAN comparison") {
        return;
    }

    if let Some(clan_output) = run_clan_to_chat(clan_command, &fixture, clan_args) {
        insta::assert_snapshot!(clan_snapshot, &clan_output);
    }
}

fn run_from_chat_case(
    corpus_path: &str,
    clan_command: Option<&str>,
    clan_args: &[&str],
    rust_snapshot: &str,
    clan_snapshot: Option<&str>,
    rust_output: impl FnOnce(&ChatFile) -> String,
) {
    let file = corpus_file(corpus_path);
    let chat_file = load_chat_file(&file);
    let rust_output = rust_output(&chat_file);
    insta::assert_snapshot!(rust_snapshot, &rust_output);

    let (Some(clan_command), Some(clan_snapshot)) = (clan_command, clan_snapshot) else {
        return;
    };

    if !require_clan_command(clan_command, "skipping CLAN comparison") {
        return;
    }

    if let Some(clan_output) = run_clan_from_chat(clan_command, &file, clan_args) {
        insta::assert_snapshot!(clan_snapshot, &clan_output);
    }
}

macro_rules! to_chat_converter_tests {
    ($($name:ident => {
        fixture: $fixture:expr,
        clan_command: $clan_command:expr,
        clan_args: $clan_args:expr,
        rust_snapshot: $rust_snapshot:expr,
        clan_snapshot: $clan_snapshot:expr,
        rust: |$content:ident| $rust:expr
    };)+) => {
        $(
            #[test]
            fn $name() {
                run_to_chat_case(
                    $fixture,
                    $clan_command,
                    $clan_args,
                    $rust_snapshot,
                    $clan_snapshot,
                    |$content| $rust,
                );
            }
        )+
    };
}

macro_rules! from_chat_converter_tests {
    ($($name:ident => {
        file: $file:expr,
        clan_command: $clan_command:expr,
        clan_args: $clan_args:expr,
        rust_snapshot: $rust_snapshot:expr,
        clan_snapshot: $clan_snapshot:expr,
        rust: |$chat_file:ident| $rust:expr
    };)+) => {
        $(
            #[test]
            fn $name() {
                run_from_chat_case(
                    $file,
                    $clan_command,
                    $clan_args,
                    $rust_snapshot,
                    $clan_snapshot,
                    |$chat_file| $rust,
                );
            }
        )+
    };
}

to_chat_converter_tests! {
    golden_srt2chat => {
        fixture: "sample.srt",
        clan_command: Some("srt2chat"),
        clan_args: &[],
        rust_snapshot: "srt2chat@rust",
        clan_snapshot: Some("srt2chat@clan"),
        rust: |content| {
            use clan_core::converters::srt2chat::srt_to_chat;

            srt_to_chat(content)
                .expect("srt_to_chat failed")
                .to_chat_string()
        }
    };
    golden_elan2chat => {
        fixture: "sample.eaf",
        clan_command: Some("elan2chat"),
        clan_args: &[],
        rust_snapshot: "elan2chat@rust",
        clan_snapshot: Some("elan2chat@clan"),
        rust: |content| {
            use clan_core::converters::elan2chat::elan_to_chat;

            elan_to_chat(content)
                .expect("elan_to_chat failed")
                .to_chat_string()
        }
    };
    golden_praat2chat => {
        fixture: "sample.TextGrid",
        clan_command: Some("praat2chat"),
        clan_args: &[],
        rust_snapshot: "praat2chat@rust",
        clan_snapshot: Some("praat2chat@clan"),
        rust: |content| {
            use clan_core::converters::praat2chat::praat_to_chat;

            praat_to_chat(content)
                .expect("praat_to_chat failed")
                .to_chat_string()
        }
    };
    golden_salt2chat => {
        fixture: "sample.salt",
        clan_command: Some("salt2chat"),
        clan_args: &[],
        rust_snapshot: "salt2chat@rust",
        clan_snapshot: Some("salt2chat@clan"),
        rust: |content| {
            use clan_core::converters::salt2chat::salt_to_chat;

            salt_to_chat(content)
                .expect("salt_to_chat failed")
                .to_chat_string()
        }
    };
    golden_text2chat => {
        fixture: "sample.txt",
        clan_command: Some("text2chat"),
        clan_args: &[],
        rust_snapshot: "text2chat@rust",
        clan_snapshot: Some("text2chat@clan"),
        rust: |content| {
            use clan_core::converters::text2chat::text_to_chat;

            text_to_chat(content)
                .expect("text_to_chat failed")
                .to_chat_string()
        }
    };
    golden_lena2chat => {
        fixture: "sample.its",
        clan_command: Some("lena2chat"),
        clan_args: &[],
        rust_snapshot: "lena2chat@rust",
        clan_snapshot: Some("lena2chat@clan"),
        rust: |content| {
            use clan_core::converters::lena2chat::lena_to_chat;

            lena_to_chat(content)
                .expect("lena_to_chat failed")
                .to_chat_string()
        }
    };
    golden_lipp2chat => {
        fixture: "sample.lipp",
        clan_command: Some("lipp2chat"),
        clan_args: &[],
        rust_snapshot: "lipp2chat@rust",
        clan_snapshot: Some("lipp2chat@clan"),
        rust: |content| {
            use clan_core::converters::lipp2chat::lipp_to_chat;

            lipp_to_chat(content)
                .expect("lipp_to_chat failed")
                .to_chat_string()
        }
    };
    golden_play2chat => {
        fixture: "sample.play",
        clan_command: Some("play2chat"),
        clan_args: &[],
        rust_snapshot: "play2chat@rust",
        clan_snapshot: Some("play2chat@clan"),
        rust: |content| {
            use clan_core::converters::play2chat::play_to_chat;

            play_to_chat(content)
                .expect("play_to_chat failed")
                .to_chat_string()
        }
    };
    golden_lab2chat => {
        fixture: "sample.lab",
        clan_command: None,
        clan_args: &[],
        rust_snapshot: "lab2chat@rust",
        clan_snapshot: None,
        rust: |content| {
            use clan_core::converters::lab2chat::lab_to_chat;

            lab_to_chat(content)
                .expect("lab_to_chat failed")
                .to_chat_string()
        }
    };
    golden_rtf2chat => {
        fixture: "sample.rtf",
        clan_command: None,
        clan_args: &[],
        rust_snapshot: "rtf2chat@rust",
        clan_snapshot: None,
        rust: |content| {
            use clan_core::converters::rtf2chat::rtf_to_chat;

            rtf_to_chat(content)
                .expect("rtf_to_chat failed")
                .to_chat_string()
        }
    };
}

from_chat_converter_tests! {
    golden_chat2srt => {
        file: "content/media-bullets.cha",
        clan_command: None,
        clan_args: &[],
        rust_snapshot: "chat2srt@rust",
        clan_snapshot: None,
        rust: |chat_file| {
            use clan_core::converters::chat2srt::chat_to_srt;

            chat_to_srt(chat_file).expect("chat_to_srt failed")
        }
    };
    golden_chat2praat => {
        file: "content/media-bullets.cha",
        clan_command: Some("chat2praat"),
        clan_args: &["+e.wav"],
        rust_snapshot: "chat2praat@rust",
        clan_snapshot: Some("chat2praat@clan"),
        rust: |chat_file| {
            use clan_core::converters::praat2chat::chat_to_praat;

            chat_to_praat(chat_file).expect("chat_to_praat failed")
        }
    };
    golden_chat2text => {
        file: "core/basic-conversation.cha",
        clan_command: None,
        clan_args: &[],
        rust_snapshot: "chat2text@rust",
        clan_snapshot: None,
        rust: |chat_file| {
            use clan_core::converters::chat2text::chat_to_text;

            chat_to_text(chat_file).expect("chat_to_text failed")
        }
    };
    golden_chat2text_with_speakers => {
        file: "core/basic-conversation.cha",
        clan_command: None,
        clan_args: &[],
        rust_snapshot: "chat2text_speakers@rust",
        clan_snapshot: None,
        rust: |chat_file| {
            use clan_core::converters::chat2text::{Chat2TextOptions, chat_to_text_with_options};

            chat_to_text_with_options(
                chat_file,
                &Chat2TextOptions {
                    include_speaker: true,
                },
            )
            .expect("chat_to_text_with_options failed")
        }
    };
    golden_chat2elan => {
        file: "content/media-bullets.cha",
        clan_command: Some("chat2elan"),
        clan_args: &["+e.wav"],
        rust_snapshot: "chat2elan@rust",
        clan_snapshot: Some("chat2elan@clan"),
        rust: |chat_file| {
            use clan_core::converters::chat2elan::chat_to_elan;

            chat_to_elan(chat_file).expect("chat_to_elan failed")
        }
    };
}
