use crate::cli::args::InstallArgs;
use crate::cli::output::Output;
use crate::cli::runtime::CliRuntime;
use engine::actions::install::InstallRequest;

pub fn run<R, O>(args: InstallArgs, runtime: &mut R, output: &mut O) -> i32
where
    R: CliRuntime,
    O: Output,
{
    let install_request = InstallRequest { tool: args.tool };

    match runtime.install(install_request) {
        Ok(res) => {
            if let Some(binary_path) = &res.binary_path {
                output.info(&format!("Binary installed at: {}", binary_path.display()));
            } else {
                output.warning(&format!(
                    "Could not find binary in {}",
                    res.install_path.display()
                ));
            }

            output.success(&format!(
                "Successfully installed {}@{} to {}",
                res.tool_name,
                res.version,
                res.install_path.display()
            ));
            0
        }
        Err(e) => {
            output.error(&format!("install failed: {e}"));
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use anyhow::anyhow;
    use engine::actions::install::{InstallRequest, InstallResult};

    use super::*;
    use crate::cli::output::BufferedOutput;

    #[derive(Default)]
    struct FakeRuntime {
        install_result: Option<anyhow::Result<InstallResult>>,
        install_requests: Vec<(String, String)>,
    }

    impl CliRuntime for FakeRuntime {
        fn install(&mut self, request: InstallRequest) -> anyhow::Result<InstallResult> {
            self.install_requests
                .push((request.tool.name, request.tool.version));
            self.install_result
                .take()
                .expect("test runtime install result was not configured")
        }
    }

    #[test]
    fn install_success_writes_stdout_and_records_request() {
        let args = install_args("ripgrep");
        let mut runtime = FakeRuntime {
            install_result: Some(Ok(InstallResult {
                tool_name: "ripgrep".to_string(),
                version: "14.1.1".to_string(),
                install_path: PathBuf::from("/opt/still/tools/ripgrep/14.1.1"),
                binary_path: Some(PathBuf::from("/opt/still/tools/ripgrep/14.1.1/bin/rg")),
            })),
            ..FakeRuntime::default()
        };
        let mut output = BufferedOutput::default();

        let code = run(args, &mut runtime, &mut output);

        assert_eq!(code, 0);
        assert_eq!(
            runtime.install_requests,
            vec![("ripgrep".to_string(), "latest".to_string())]
        );
        insta::assert_snapshot!(output.stdout, @r###"
Binary installed at: /opt/still/tools/ripgrep/14.1.1/bin/rg
✓ Successfully installed ripgrep@14.1.1 to /opt/still/tools/ripgrep/14.1.1
"###);
        assert_eq!(output.stderr, "");
    }

    #[test]
    fn install_success_without_binary_writes_warning_to_stderr() {
        let args = install_args("ripgrep");
        let mut runtime = FakeRuntime {
            install_result: Some(Ok(InstallResult {
                tool_name: "ripgrep".to_string(),
                version: "14.1.1".to_string(),
                install_path: PathBuf::from("/opt/still/tools/ripgrep/14.1.1"),
                binary_path: None,
            })),
            ..FakeRuntime::default()
        };
        let mut output = BufferedOutput::default();

        let code = run(args, &mut runtime, &mut output);

        assert_eq!(code, 0);
        insta::assert_snapshot!(output.stdout, @r###"
✓ Successfully installed ripgrep@14.1.1 to /opt/still/tools/ripgrep/14.1.1
"###);
        insta::assert_snapshot!(output.stderr, @r###"
⚠ Could not find binary in /opt/still/tools/ripgrep/14.1.1
"###);
    }

    #[test]
    fn install_error_writes_stderr_and_returns_nonzero() {
        let args = install_args("ripgrep");
        let mut runtime = FakeRuntime {
            install_result: Some(Err(anyhow!("formula.json not found"))),
            ..FakeRuntime::default()
        };
        let mut output = BufferedOutput::default();

        let code = run(args, &mut runtime, &mut output);

        assert_eq!(code, 1);
        assert_eq!(output.stdout, "");
        insta::assert_snapshot!(output.stderr, @r###"
install failed: formula.json not found
"###);
    }

    fn install_args(tool: &str) -> InstallArgs {
        InstallArgs {
            tool: tool.parse().expect("test tool spec should parse"),
        }
    }
}
