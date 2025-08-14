use serde::{Deserialize, Serialize};
use tracing::{info, instrument};
use utoipa::ToSchema;

const SUBSTITUTION_VAR_NAME: &str = "CHANGING_TEST";
const PASSKEY_VAR_NAME: &str = "PASSKEY_PATH";
#[derive(Debug)]
pub struct TestExecutor {}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CmdOutput {
    pub status: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl TestExecutor {
    #[instrument(level = "trace", skip(substitute_cmd, passkey_path), fields(command_to_substitute = substitute_cmd.as_ref(), passkey_path = passkey_path.as_ref()))]
    pub fn execute<S: AsRef<str>, P: AsRef<str>>(
        substitute_cmd: S,
        passkey_path: P,
    ) -> crate::error::Result<CmdOutput> {
        info!("Making some work...");
        Ok(CmdOutput {
            status: None,
            stdout: "all ok".to_string(),
            stderr: "all ok".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use global_utils::logger::init_logger;

    use crate::test_executor::TestExecutor;

    #[test]
    fn invocation() -> anyhow::Result<()> {
        dotenv::dotenv()?;
        let x = init_logger();
        println!(
            "{:#?}",
            TestExecutor::execute(
                "./sdks/js/packages/artillery/config/scenarios/token-announce-with-identity.yml",
                ""
            )?
        );
        Ok(())
    }
}
