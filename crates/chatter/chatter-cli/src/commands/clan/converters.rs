use crate::cli::ClanCommands;
use clan_core::converters::{
    chat2elan, chat2srt, chat2text, elan2chat, lab2chat, lena2chat, lipp2chat, play2chat,
    praat2chat, rtf2chat, salt2chat, srt2chat, text2chat,
};

use super::helpers::{
    exit_with_error, parse_chat_or_exit, read_file_or_exit, run_converter, write_output_or_exit,
};

pub(super) fn dispatch(command: ClanCommands) -> Result<(), ClanCommands> {
    match command {
        ClanCommands::Srt2chat {
            path,
            language,
            output,
        } => {
            let content = read_file_or_exit(&path);
            run_converter(
                srt2chat::srt_to_chat_with_options(&content, "SPK", &language, "srt_corpus"),
                output.as_deref(),
            );
        }
        ClanCommands::Chat2srt { path, output } => {
            let chat = parse_chat_or_exit(&path);
            match chat2srt::chat_to_srt(&chat) {
                Ok(srt) => write_output_or_exit(&srt, output.as_deref()),
                Err(e) => exit_with_error(format!("Error: {e}")),
            }
        }
        ClanCommands::Chat2vtt { path, output } => {
            let chat = parse_chat_or_exit(&path);
            match chat2srt::chat_to_vtt(&chat) {
                Ok(vtt) => write_output_or_exit(&vtt, output.as_deref()),
                Err(e) => exit_with_error(format!("Error: {e}")),
            }
        }
        ClanCommands::Text2chat {
            path,
            speaker,
            language,
            output,
        } => {
            let content = read_file_or_exit(&path);
            run_converter(
                text2chat::text_to_chat_with_options(&content, &speaker, &language, "text_corpus"),
                output.as_deref(),
            );
        }
        ClanCommands::Lipp2chat {
            path,
            speaker,
            language,
            output,
        } => {
            let content = read_file_or_exit(&path);
            run_converter(
                lipp2chat::lipp_to_chat_with_options(&content, &speaker, &language, "lipp_corpus"),
                output.as_deref(),
            );
        }
        ClanCommands::Elan2chat {
            path,
            language,
            output,
        } => {
            let content = read_file_or_exit(&path);
            run_converter(
                elan2chat::elan_to_chat_with_options(&content, &language, "elan_corpus"),
                output.as_deref(),
            );
        }
        ClanCommands::Praat2chat {
            path,
            language,
            output,
        } => {
            let content = read_file_or_exit(&path);
            run_converter(
                praat2chat::praat_to_chat_with_options(&content, &language, "praat_corpus"),
                output.as_deref(),
            );
        }
        ClanCommands::Chat2praat { path, output } => {
            let chat = parse_chat_or_exit(&path);
            match praat2chat::chat_to_praat(&chat) {
                Ok(tg) => write_output_or_exit(&tg, output.as_deref()),
                Err(e) => exit_with_error(format!("Error: {e}")),
            }
        }
        ClanCommands::Lena2chat {
            path,
            language,
            output,
        } => {
            let content = read_file_or_exit(&path);
            run_converter(
                lena2chat::lena_to_chat_with_options(&content, &language, "lena_corpus"),
                output.as_deref(),
            );
        }
        ClanCommands::Play2chat {
            path,
            language,
            output,
        } => {
            let content = read_file_or_exit(&path);
            run_converter(
                play2chat::play_to_chat_with_options(&content, &language, "play_corpus"),
                output.as_deref(),
            );
        }
        ClanCommands::Lab2chat {
            path,
            speaker,
            language,
            output,
        } => {
            let content = read_file_or_exit(&path);
            run_converter(
                lab2chat::lab_to_chat_with_options(&content, &speaker, &language, "lab_corpus"),
                output.as_deref(),
            );
        }
        ClanCommands::Rtf2chat {
            path,
            language,
            output,
        } => {
            let content = read_file_or_exit(&path);
            run_converter(
                rtf2chat::rtf_to_chat_with_options(&content, &language, "rtf_corpus"),
                output.as_deref(),
            );
        }
        ClanCommands::Salt2chat {
            path,
            language,
            output,
        } => {
            let content = read_file_or_exit(&path);
            run_converter(
                salt2chat::salt_to_chat_with_options(&content, &language, "salt_corpus"),
                output.as_deref(),
            );
        }
        ClanCommands::Chat2text {
            path,
            include_speaker,
            output,
        } => {
            let chat = parse_chat_or_exit(&path);
            let options = chat2text::Chat2TextOptions { include_speaker };
            match chat2text::chat_to_text_with_options(&chat, &options) {
                Ok(text) => write_output_or_exit(&text, output.as_deref()),
                Err(e) => exit_with_error(format!("Error: {e}")),
            }
        }
        ClanCommands::Chat2elan {
            path,
            media_extension,
            output,
        } => {
            let chat = parse_chat_or_exit(&path);
            match chat2elan::chat_to_elan_with_options(&chat, media_extension.as_deref()) {
                Ok(eaf) => write_output_or_exit(&eaf, output.as_deref()),
                Err(e) => exit_with_error(format!("Error: {e}")),
            }
        }
        other => return Err(other),
    }
    Ok(())
}
