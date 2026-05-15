use talkbank_model::ChatOptionFlag;
use talkbank_model::ErrorCollector;
use talkbank_model::FragmentSemanticContext;
use talkbank_parser::TreeSitterParser;

fn parser() -> TreeSitterParser {
    TreeSitterParser::new().expect("grammar loads")
}

#[test]
fn public_main_tier_wrapper_rejects_ca_fragment_without_context() {
    let p = parser();
    let errors = ErrorCollector::new();
    let result = p.parse_main_tier_fragment("*CHI:\t(word) .", 0, &errors);
    assert!(result.into_option().is_none() || !errors.is_empty());
}

#[test]
fn public_main_tier_wrapper_accepts_ca_fragment_with_context() {
    let p = parser();
    let context = FragmentSemanticContext::new().with_option_flag(ChatOptionFlag::Ca);
    let errors = ErrorCollector::new();
    let result = p.parse_main_tier_fragment_with_context("*CHI:\t(word) .", 0, &context, &errors);
    assert!(result.into_option().is_some());
}

#[test]
fn public_utterance_wrapper_rejects_ca_fragment_without_context() {
    let p = parser();
    let errors = ErrorCollector::new();
    let result = p.parse_utterance_fragment("*CHI:\t(word) .\n%mor:\tv|(word) .\n", 0, &errors);
    assert!(result.into_option().is_none() || !errors.is_empty());
}

#[test]
fn public_utterance_wrapper_accepts_ca_fragment_with_context() {
    let p = parser();
    let context = FragmentSemanticContext::new().with_option_flag(ChatOptionFlag::Ca);
    let errors = ErrorCollector::new();
    let result = p.parse_utterance_fragment_with_context("*CHI:\t(word) .\n", 0, &context, &errors);
    assert!(result.into_option().is_some());
}
