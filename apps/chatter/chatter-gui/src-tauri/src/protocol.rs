//! Shared desktop command and event contracts.
//!
//! TypeScript mirrors live in `desktop/src/protocol/desktopProtocol.ts`.

use serde::{Deserialize, Serialize};

pub mod commands {
    use super::*;

    pub const VALIDATE: &str = "validate";
    pub const CANCEL_VALIDATION: &str = "cancel_validation";
    pub const CHECK_CLAN_AVAILABLE: &str = "check_clan_available";
    pub const OPEN_IN_CLAN: &str = "open_in_clan";
    pub const EXPORT_RESULTS: &str = "export_results";
    pub const REVEAL_IN_FILE_MANAGER: &str = "reveal_in_file_manager";
    pub const INSTALL_CLI: &str = "install_cli";

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct ValidateRequest {
        pub path: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct OpenInClanRequest {
        pub file: String,
        pub line: i32,
        pub col: i32,
        pub byte_offset: u32,
        pub msg: String,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "lowercase")]
    pub enum ExportFormat {
        Json,
        Text,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct ExportResultsRequest {
        pub results: String,
        pub format: ExportFormat,
        pub path: String,
    }
}

pub mod events {
    pub const VALIDATION: &str = "validation-event";
}
