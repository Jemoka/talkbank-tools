//! CLAN analysis service — dispatches commands to handlers.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::commands::chains::ChainsCommand;
use crate::commands::chip::ChipCommand;
use crate::commands::codes::CodesCommand;
use crate::commands::combo::ComboCommand;
use crate::commands::complexity::ComplexityCommand;
use crate::commands::cooccur::CooccurCommand;
use crate::commands::corelex::CorelexCommand;
use crate::commands::dist::DistCommand;
use crate::commands::dss::DssCommand;
use crate::commands::eval::EvalCommand;
use crate::commands::flucalc::FlucalcCommand;
use crate::commands::freq::FreqCommand;
use crate::commands::freqpos::FreqposCommand;
use crate::commands::gemlist::GemlistCommand;
use crate::commands::ipsyn::IpsynCommand;
use crate::commands::keymap::KeymapCommand;
use crate::commands::kideval::KidevalCommand;
use crate::commands::kwal::KwalCommand;
use crate::commands::maxwd::MaxwdCommand;
use crate::commands::mlt::MltCommand;
use crate::commands::mlu::MluCommand;
use crate::commands::modrep::ModrepCommand;
use crate::commands::mortable::MortableCommand;
use crate::commands::phonfreq::PhonfreqCommand;
use crate::commands::rely::run_rely;
use crate::commands::script::ScriptCommand;
use crate::commands::sugar::SugarCommand;
use crate::commands::timedur::TimedurCommand;
use crate::commands::trnfix::TrnfixCommand;
use crate::commands::uniq::UniqCommand;
use crate::commands::vocd::VocdCommand;
use crate::commands::wdlen::WdlenCommand;
use crate::commands::wdsize::WdsizeCommand;
use crate::framework::{
    AnalysisCommand, AnalysisRunner, CommandOutput, FilterConfig, OutputFormat,
};

use super::service_types::*;

/// Entry point for running CLAN analysis commands on CHAT files.
pub struct AnalysisService {
    runner: AnalysisRunner,
}

impl AnalysisService {
    /// Construct a service with default pass-through filtering.
    pub fn new() -> Self {
        Self {
            runner: AnalysisRunner::new(),
        }
    }

    /// Construct a service with the given filter configuration.
    pub fn with_filter(filter: FilterConfig) -> Self {
        Self {
            runner: AnalysisRunner::with_filter(filter),
        }
    }

    /// Execute one analysis request and return structured JSON output.
    pub fn execute_json(
        &self,
        request: AnalysisRequest,
        files: &[PathBuf],
    ) -> Result<Value, AnalysisServiceError> {
        match request {
            AnalysisRequest::Freq(config) => self.run_json(&FreqCommand::new(config), files),
            AnalysisRequest::Mlu(config) => self.run_json(&MluCommand::new(config), files),
            AnalysisRequest::Mlt => self.run_json(&MltCommand, files),
            AnalysisRequest::Wdlen => self.run_json(&WdlenCommand, files),
            AnalysisRequest::Wdsize(config) => self.run_json(&WdsizeCommand::new(config), files),
            AnalysisRequest::Maxwd(config) => self.run_json(&MaxwdCommand::new(config), files),
            AnalysisRequest::Freqpos => self.run_json(&FreqposCommand, files),
            AnalysisRequest::Timedur => self.run_json(&TimedurCommand, files),
            AnalysisRequest::Kwal(config) => self.run_json(&KwalCommand::new(config), files),
            AnalysisRequest::Gemlist => self.run_json(&GemlistCommand, files),
            AnalysisRequest::Combo(config) => self.run_json(&ComboCommand::new(config), files),
            AnalysisRequest::Cooccur => self.run_json(&CooccurCommand, files),
            AnalysisRequest::Dist => self.run_json(&DistCommand, files),
            AnalysisRequest::Chip => self.run_json(&ChipCommand, files),
            AnalysisRequest::Phonfreq => self.run_json(&PhonfreqCommand, files),
            AnalysisRequest::Modrep => self.run_json(&ModrepCommand, files),
            AnalysisRequest::Vocd => self.run_json(&VocdCommand::default(), files),
            AnalysisRequest::Codes(config) => self.run_json(&CodesCommand::new(config), files),
            AnalysisRequest::Chains(config) => self.run_json(&ChainsCommand::new(config), files),
            AnalysisRequest::Complexity => self.run_json(&ComplexityCommand, files),
            AnalysisRequest::Corelex(config) => self.run_json(&CorelexCommand::new(config), files),
            AnalysisRequest::Dss(config) => {
                let command = DssCommand::new(config)
                    .map_err(|error| AnalysisServiceError::InvalidRequest(error.to_string()))?;
                self.run_json(&command, files)
            }
            AnalysisRequest::Eval(config) => self.run_json(&EvalCommand::new(config), files),
            AnalysisRequest::Flucalc(config) => self.run_json(&FlucalcCommand::new(config), files),
            AnalysisRequest::Ipsyn(config) => {
                let command = IpsynCommand::new(config)
                    .map_err(|error| AnalysisServiceError::InvalidRequest(error.to_string()))?;
                self.run_json(&command, files)
            }
            AnalysisRequest::Keymap(config) => self.run_json(&KeymapCommand::new(config), files),
            AnalysisRequest::Kideval(config) => {
                let command = KidevalCommand::new(config)
                    .map_err(|error| AnalysisServiceError::InvalidRequest(error.to_string()))?;
                self.run_json(&command, files)
            }
            AnalysisRequest::Mortable(config) => {
                let command = MortableCommand::new(config)
                    .map_err(|error| AnalysisServiceError::InvalidRequest(error.to_string()))?;
                self.run_json(&command, files)
            }
            AnalysisRequest::Script(config) => {
                let command = ScriptCommand::new(config)
                    .map_err(|error| AnalysisServiceError::InvalidRequest(error.to_string()))?;
                self.run_json(&command, files)
            }
            AnalysisRequest::Sugar(config) => self.run_json(&SugarCommand::new(config), files),
            AnalysisRequest::Trnfix(config) => self.run_json(&TrnfixCommand::new(config), files),
            AnalysisRequest::Uniq(config) => self.run_json(&UniqCommand::new(config), files),
        }
    }

    /// Execute one `rely` request and return structured JSON output.
    pub fn execute_rely_json(
        &self,
        request: RelyRequest,
        primary_file: &Path,
    ) -> Result<Value, AnalysisServiceError> {
        let result = run_rely(&request.config, primary_file, &request.secondary_file)?;
        serde_json::to_value(&result).map_err(|error| {
            AnalysisServiceError::InvalidRequest(format!(
                "Failed to serialize rely result: {error}"
            ))
        })
    }

    /// Execute one analysis request and render aggregate output in the requested format.
    pub fn execute_rendered(
        &self,
        request: AnalysisRequest,
        files: &[PathBuf],
        format: OutputFormat,
    ) -> Result<String, AnalysisServiceError> {
        match request {
            AnalysisRequest::Freq(config) => {
                self.run_rendered(&FreqCommand::new(config), files, format)
            }
            AnalysisRequest::Mlu(config) => {
                self.run_rendered(&MluCommand::new(config), files, format)
            }
            AnalysisRequest::Mlt => self.run_rendered(&MltCommand, files, format),
            AnalysisRequest::Wdlen => self.run_rendered(&WdlenCommand, files, format),
            AnalysisRequest::Wdsize(config) => {
                self.run_rendered(&WdsizeCommand::new(config), files, format)
            }
            AnalysisRequest::Maxwd(config) => {
                self.run_rendered(&MaxwdCommand::new(config), files, format)
            }
            AnalysisRequest::Freqpos => self.run_rendered(&FreqposCommand, files, format),
            AnalysisRequest::Timedur => self.run_rendered(&TimedurCommand, files, format),
            AnalysisRequest::Kwal(config) => {
                self.run_rendered(&KwalCommand::new(config), files, format)
            }
            AnalysisRequest::Gemlist => self.run_rendered(&GemlistCommand, files, format),
            AnalysisRequest::Combo(config) => {
                self.run_rendered(&ComboCommand::new(config), files, format)
            }
            AnalysisRequest::Cooccur => self.run_rendered(&CooccurCommand, files, format),
            AnalysisRequest::Dist => self.run_rendered(&DistCommand, files, format),
            AnalysisRequest::Chip => self.run_rendered(&ChipCommand, files, format),
            AnalysisRequest::Phonfreq => self.run_rendered(&PhonfreqCommand, files, format),
            AnalysisRequest::Modrep => self.run_rendered(&ModrepCommand, files, format),
            AnalysisRequest::Vocd => self.run_rendered(&VocdCommand::default(), files, format),
            AnalysisRequest::Codes(config) => {
                self.run_rendered(&CodesCommand::new(config), files, format)
            }
            AnalysisRequest::Chains(config) => {
                self.run_rendered(&ChainsCommand::new(config), files, format)
            }
            AnalysisRequest::Complexity => self.run_rendered(&ComplexityCommand, files, format),
            AnalysisRequest::Corelex(config) => {
                self.run_rendered(&CorelexCommand::new(config), files, format)
            }
            AnalysisRequest::Dss(config) => {
                let command = DssCommand::new(config)
                    .map_err(|error| AnalysisServiceError::InvalidRequest(error.to_string()))?;
                self.run_rendered(&command, files, format)
            }
            AnalysisRequest::Eval(config) => {
                self.run_rendered(&EvalCommand::new(config), files, format)
            }
            AnalysisRequest::Flucalc(config) => {
                self.run_rendered(&FlucalcCommand::new(config), files, format)
            }
            AnalysisRequest::Ipsyn(config) => {
                let command = IpsynCommand::new(config)
                    .map_err(|error| AnalysisServiceError::InvalidRequest(error.to_string()))?;
                self.run_rendered(&command, files, format)
            }
            AnalysisRequest::Keymap(config) => {
                self.run_rendered(&KeymapCommand::new(config), files, format)
            }
            AnalysisRequest::Kideval(config) => {
                let command = KidevalCommand::new(config)
                    .map_err(|error| AnalysisServiceError::InvalidRequest(error.to_string()))?;
                self.run_rendered(&command, files, format)
            }
            AnalysisRequest::Mortable(config) => {
                let command = MortableCommand::new(config)
                    .map_err(|error| AnalysisServiceError::InvalidRequest(error.to_string()))?;
                self.run_rendered(&command, files, format)
            }
            AnalysisRequest::Script(config) => {
                let command = ScriptCommand::new(config)
                    .map_err(|error| AnalysisServiceError::InvalidRequest(error.to_string()))?;
                self.run_rendered(&command, files, format)
            }
            AnalysisRequest::Sugar(config) => {
                self.run_rendered(&SugarCommand::new(config), files, format)
            }
            AnalysisRequest::Trnfix(config) => {
                self.run_rendered(&TrnfixCommand::new(config), files, format)
            }
            AnalysisRequest::Uniq(config) => {
                self.run_rendered(&UniqCommand::new(config), files, format)
            }
        }
    }

    /// Execute one `rely` request and render output in the requested format.
    pub fn execute_rely_rendered(
        &self,
        request: RelyRequest,
        primary_file: &Path,
        format: OutputFormat,
    ) -> Result<String, AnalysisServiceError> {
        let result = run_rely(&request.config, primary_file, &request.secondary_file)?;
        Ok(result.render(format))
    }

    /// Execute one analysis request in per-file mode and render each result.
    pub fn execute_rendered_per_file(
        &self,
        request: AnalysisRequest,
        files: &[PathBuf],
        format: OutputFormat,
    ) -> Result<Vec<(PathBuf, String)>, AnalysisServiceError> {
        match request {
            AnalysisRequest::Freq(config) => {
                self.run_rendered_per_file(&FreqCommand::new(config), files, format)
            }
            AnalysisRequest::Mlu(config) => {
                self.run_rendered_per_file(&MluCommand::new(config), files, format)
            }
            AnalysisRequest::Mlt => self.run_rendered_per_file(&MltCommand, files, format),
            AnalysisRequest::Wdlen => self.run_rendered_per_file(&WdlenCommand, files, format),
            AnalysisRequest::Wdsize(config) => {
                self.run_rendered_per_file(&WdsizeCommand::new(config), files, format)
            }
            AnalysisRequest::Maxwd(config) => {
                self.run_rendered_per_file(&MaxwdCommand::new(config), files, format)
            }
            AnalysisRequest::Freqpos => self.run_rendered_per_file(&FreqposCommand, files, format),
            AnalysisRequest::Timedur => self.run_rendered_per_file(&TimedurCommand, files, format),
            AnalysisRequest::Kwal(config) => {
                self.run_rendered_per_file(&KwalCommand::new(config), files, format)
            }
            AnalysisRequest::Gemlist => self.run_rendered_per_file(&GemlistCommand, files, format),
            AnalysisRequest::Combo(config) => {
                self.run_rendered_per_file(&ComboCommand::new(config), files, format)
            }
            AnalysisRequest::Cooccur => self.run_rendered_per_file(&CooccurCommand, files, format),
            AnalysisRequest::Dist => self.run_rendered_per_file(&DistCommand, files, format),
            AnalysisRequest::Chip => self.run_rendered_per_file(&ChipCommand, files, format),
            AnalysisRequest::Phonfreq => {
                self.run_rendered_per_file(&PhonfreqCommand, files, format)
            }
            AnalysisRequest::Modrep => self.run_rendered_per_file(&ModrepCommand, files, format),
            AnalysisRequest::Vocd => {
                self.run_rendered_per_file(&VocdCommand::default(), files, format)
            }
            AnalysisRequest::Codes(config) => {
                self.run_rendered_per_file(&CodesCommand::new(config), files, format)
            }
            AnalysisRequest::Chains(config) => {
                self.run_rendered_per_file(&ChainsCommand::new(config), files, format)
            }
            AnalysisRequest::Complexity => {
                self.run_rendered_per_file(&ComplexityCommand, files, format)
            }
            AnalysisRequest::Corelex(config) => {
                self.run_rendered_per_file(&CorelexCommand::new(config), files, format)
            }
            AnalysisRequest::Dss(config) => {
                let command = DssCommand::new(config)
                    .map_err(|error| AnalysisServiceError::InvalidRequest(error.to_string()))?;
                self.run_rendered_per_file(&command, files, format)
            }
            AnalysisRequest::Eval(config) => {
                self.run_rendered_per_file(&EvalCommand::new(config), files, format)
            }
            AnalysisRequest::Flucalc(config) => {
                self.run_rendered_per_file(&FlucalcCommand::new(config), files, format)
            }
            AnalysisRequest::Ipsyn(config) => {
                let command = IpsynCommand::new(config)
                    .map_err(|error| AnalysisServiceError::InvalidRequest(error.to_string()))?;
                self.run_rendered_per_file(&command, files, format)
            }
            AnalysisRequest::Keymap(config) => {
                self.run_rendered_per_file(&KeymapCommand::new(config), files, format)
            }
            AnalysisRequest::Kideval(config) => {
                let command = KidevalCommand::new(config)
                    .map_err(|error| AnalysisServiceError::InvalidRequest(error.to_string()))?;
                self.run_rendered_per_file(&command, files, format)
            }
            AnalysisRequest::Mortable(config) => {
                let command = MortableCommand::new(config)
                    .map_err(|error| AnalysisServiceError::InvalidRequest(error.to_string()))?;
                self.run_rendered_per_file(&command, files, format)
            }
            AnalysisRequest::Script(config) => {
                let command = ScriptCommand::new(config)
                    .map_err(|error| AnalysisServiceError::InvalidRequest(error.to_string()))?;
                self.run_rendered_per_file(&command, files, format)
            }
            AnalysisRequest::Sugar(config) => {
                self.run_rendered_per_file(&SugarCommand::new(config), files, format)
            }
            AnalysisRequest::Trnfix(config) => {
                self.run_rendered_per_file(&TrnfixCommand::new(config), files, format)
            }
            AnalysisRequest::Uniq(config) => {
                self.run_rendered_per_file(&UniqCommand::new(config), files, format)
            }
        }
    }

    fn run_json<C: AnalysisCommand>(
        &self,
        command: &C,
        files: &[PathBuf],
    ) -> Result<Value, AnalysisServiceError>
    where
        C::Output: CommandOutput,
    {
        let output = self.runner.run(command, files)?;
        Ok(output.to_json_value())
    }

    fn run_rendered<C: AnalysisCommand>(
        &self,
        command: &C,
        files: &[PathBuf],
        format: OutputFormat,
    ) -> Result<String, AnalysisServiceError>
    where
        C::Output: CommandOutput,
    {
        let output = self.runner.run(command, files)?;
        Ok(output.render(format))
    }

    fn run_rendered_per_file<C: AnalysisCommand>(
        &self,
        command: &C,
        files: &[PathBuf],
        format: OutputFormat,
    ) -> Result<Vec<(PathBuf, String)>, AnalysisServiceError>
    where
        C::Output: CommandOutput,
    {
        let outputs = self.runner.run_per_file(command, files)?;
        Ok(outputs
            .into_iter()
            .map(|(path, output)| (path, output.render(format)))
            .collect())
    }
}

impl Default for AnalysisService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::chains::ChainsConfig;
    use crate::commands::corelex::CorelexConfig;
    use crate::commands::rely::RelyConfig;
    use crate::commands::sugar::SugarConfig;
    use crate::commands::trnfix::TrnfixConfig;
    use crate::service_types::*;

    #[test]
    fn analysis_command_name_round_trips_wire_names() {
        let commands = [
            AnalysisCommandName::Freq,
            AnalysisCommandName::Mlu,
            AnalysisCommandName::Mlt,
            AnalysisCommandName::Wdlen,
            AnalysisCommandName::Wdsize,
            AnalysisCommandName::Maxwd,
            AnalysisCommandName::Freqpos,
            AnalysisCommandName::Timedur,
            AnalysisCommandName::Kwal,
            AnalysisCommandName::Gemlist,
            AnalysisCommandName::Combo,
            AnalysisCommandName::Cooccur,
            AnalysisCommandName::Dist,
            AnalysisCommandName::Chip,
            AnalysisCommandName::Phonfreq,
            AnalysisCommandName::Modrep,
            AnalysisCommandName::Vocd,
            AnalysisCommandName::Codes,
            AnalysisCommandName::Chains,
            AnalysisCommandName::Complexity,
            AnalysisCommandName::Corelex,
            AnalysisCommandName::Dss,
            AnalysisCommandName::Eval,
            AnalysisCommandName::EvalDialect,
            AnalysisCommandName::Flucalc,
            AnalysisCommandName::Ipsyn,
            AnalysisCommandName::Keymap,
            AnalysisCommandName::Kideval,
            AnalysisCommandName::Mortable,
            AnalysisCommandName::Rely,
            AnalysisCommandName::Script,
            AnalysisCommandName::Sugar,
            AnalysisCommandName::Trnfix,
            AnalysisCommandName::Uniq,
        ];

        for command in commands {
            let parsed = command
                .as_str()
                .parse::<AnalysisCommandName>()
                .expect("command name should parse");
            assert_eq!(parsed, command);
            assert_eq!(parsed.to_string(), command.as_str());
        }
    }

    #[test]
    fn analysis_command_name_rejects_unknown_strings() {
        let error = "not-real"
            .parse::<AnalysisCommandName>()
            .expect_err("unknown command should fail");
        assert_eq!(error.to_string(), "Unknown analysis command: not-real");
    }

    #[test]
    fn builder_uses_corelex_library_default() {
        let plan =
            AnalysisRequestBuilder::new(AnalysisCommandName::Corelex, AnalysisOptions::default())
                .build()
                .expect("corelex should build");

        match plan {
            AnalysisPlan::Service(AnalysisRequest::Corelex(config)) => {
                assert_eq!(config.min_frequency, CorelexConfig::default().min_frequency);
            }
            other => panic!("unexpected plan: {other:?}"),
        }
    }

    #[test]
    fn builder_uses_sugar_library_default() {
        let plan =
            AnalysisRequestBuilder::new(AnalysisCommandName::Sugar, AnalysisOptions::default())
                .build()
                .expect("sugar should build");

        match plan {
            AnalysisPlan::Service(AnalysisRequest::Sugar(config)) => {
                assert_eq!(config.min_utterances, SugarConfig::default().min_utterances);
            }
            other => panic!("unexpected plan: {other:?}"),
        }
    }

    #[test]
    fn builder_uses_default_tiers() {
        let chains =
            AnalysisRequestBuilder::new(AnalysisCommandName::Chains, AnalysisOptions::default())
                .build()
                .expect("chains should build");
        let trnfix =
            AnalysisRequestBuilder::new(AnalysisCommandName::Trnfix, AnalysisOptions::default())
                .build()
                .expect("trnfix should build");

        match chains {
            AnalysisPlan::Service(AnalysisRequest::Chains(config)) => {
                assert_eq!(config.tier, ChainsConfig::default().tier);
            }
            other => panic!("unexpected plan: {other:?}"),
        }

        match trnfix {
            AnalysisPlan::Service(AnalysisRequest::Trnfix(config)) => {
                let default = TrnfixConfig::default();
                assert_eq!(config.tier1, default.tier1);
                assert_eq!(config.tier2, default.tier2);
            }
            other => panic!("unexpected plan: {other:?}"),
        }
    }

    #[test]
    fn builder_requires_rely_second_file() {
        let error =
            AnalysisRequestBuilder::new(AnalysisCommandName::Rely, AnalysisOptions::default())
                .build()
                .expect_err("rely without second file should fail");
        assert!(matches!(
            error,
            AnalysisServiceError::InvalidRequest(message) if message.contains("secondFile")
        ));
    }

    #[test]
    fn builder_uses_rely_default_tier() {
        let options = AnalysisOptions {
            second_file: Some(PathBuf::from("/tmp/other.cha")),
            ..AnalysisOptions::default()
        };
        let plan = AnalysisRequestBuilder::new(AnalysisCommandName::Rely, options)
            .build()
            .expect("rely should build");

        match plan {
            AnalysisPlan::Rely(request) => {
                assert_eq!(request.config.tier, RelyConfig::default().tier);
            }
            other => panic!("unexpected plan: {other:?}"),
        }
    }
}
