//! QUOTES -- extract quoted text to separate utterances.
//!
//! The original CLAN command rewrites quote-extraction markers (`[+ "]`) into
//! a multi-utterance `+"/.` / `+"` sequence.
//!
//! `clan-core` does not perform that rewrite through raw CHAT string
//! surgery. Instead, this command inspects the parsed AST and:
//!
//! - emits unchanged normalized CHAT when no quote-extraction postcode exists
//! - returns an explicit error when `[+ "]` is present, because that rewrite is
//!   not yet implemented in a principled AST-based form
//!
//! This is a relatively uncommon command used for discourse analysis
//! of reported speech.

use talkbank_model::ChatFile;

use crate::framework::{TransformCommand, TransformError, run_transform};

/// QUOTES transform: extract quoted text to separate utterances.
pub struct QuotesCommand;

impl TransformCommand for QuotesCommand {
    type Config = ();

    fn transform(&self, file: &mut ChatFile) -> Result<(), TransformError> {
        for utterance in file.utterances() {
            if utterance
                .main
                .content
                .postcodes
                .iter()
                .any(|p| p.text == "\"")
            {
                return Err(TransformError::Transform(format!(
                    "QUOTES encountered unsupported quote-extraction postcode [+ \"] on speaker {}. \
                     This rewrite must be implemented through the CHAT AST before it can be supported.",
                    utterance.main.speaker
                )));
            }
        }

        Ok(())
    }
}

/// Run QUOTES through the standard AST transform pipeline.
pub fn run_quotes(
    input: &std::path::Path,
    output: Option<&std::path::Path>,
) -> Result<(), TransformError> {
    run_transform(&QuotesCommand, input, output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::Span;
    use talkbank_model::{ChatFile, Line, MainTier, Postcode, Terminator, Utterance};

    #[test]
    fn quotes_is_noop_without_quote_extraction_postcode() {
        let mut file = ChatFile::new(vec![Line::utterance(Utterance::new(MainTier::new(
            "MOT",
            vec![],
            Terminator::Period { span: Span::DUMMY },
        )))]);

        QuotesCommand
            .transform(&mut file)
            .expect("quotes should be a no-op without [+ \"]");
    }

    #[test]
    fn quotes_errors_on_unsupported_quote_extraction_postcode() {
        let mut main = MainTier::new("MOT", vec![], Terminator::Period { span: Span::DUMMY });
        main.content.postcodes.push(Postcode::new("\""));
        let mut file = ChatFile::new(vec![Line::utterance(Utterance::new(main))]);

        let err = QuotesCommand
            .transform(&mut file)
            .expect_err("quotes should fail on unsupported [+ \"]");
        let msg = err.to_string();
        assert!(msg.contains("unsupported quote-extraction postcode"));
        assert!(msg.contains("MOT"));
    }
}
