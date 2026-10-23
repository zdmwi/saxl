//! `saxl explain`: print a rule's documentation, or list every code.

use std::process::ExitCode;

use crate::CliError;

#[derive(clap::Args)]
pub struct Args {
    /// A diagnostic code such as `E006`. Lists every code when omitted.
    code: Option<String>,
}

pub fn run(args: &Args) -> Result<ExitCode, CliError> {
    match &args.code {
        Some(code) => {
            let doc = saxl_validator::explain(code).ok_or_else(|| {
                CliError(format!(
                    "unknown code `{code}`; run `saxl explain` to list every code"
                ))
            })?;
            print!("{doc}");
        }
        None => {
            for (code, summary) in saxl_validator::PARSE_CODES {
                println!("{code}  {summary}");
            }
            for rule in saxl_validator::rules() {
                println!("{}  {}", rule.code(), rule.summary());
            }
        }
    }
    Ok(ExitCode::SUCCESS)
}
