//! CLI-facing install command handler.

use crate::cli::args::InstallArgs;
use crate::cli::output::Output;
use crate::cli::runtime::{CliRuntime, InstallCommandRequest};
use engine::actions::install::{InstallItemRequest, InstallRequest};
use engine::registries::specs::tool::ToolSpec;
use engine::specs::item::{ItemKind, ItemSpec};

/// Runs the install command through the configured runtime.
///
/// `args` is the parsed CLI request and is converted into an engine
/// `InstallRequest`. `runtime` owns the actual install behavior, making the
/// command handler testable without filesystem or network work. `output` receives
/// success, warning, and error messages. Returns `0` on install success and `1`
/// when the runtime reports an install error.
pub fn run<R, O>(args: InstallArgs, runtime: &mut R, output: &mut O) -> i32
where
    R: CliRuntime,
    O: Output,
{
    let global = args.global;
    let items = match install_items(args) {
        Ok(items) => items,
        Err(e) => {
            output.error(&format!("install failed: {e}"));
            return 1;
        }
    };
    let install_request = InstallCommandRequest {
        global,
        install: InstallRequest { items },
    };

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

fn install_items(args: InstallArgs) -> anyhow::Result<Vec<InstallItemRequest>> {
    let mut items = Vec::new();
    push_items(&mut items, ItemKind::Tool, args.tools);
    push_items(&mut items, ItemKind::Package, args.packages);
    push_items(&mut items, ItemKind::App, args.apps);
    for spec in args.items {
        let kind = infer_item_kind(&spec)?;
        push_items(&mut items, kind, vec![spec]);
    }
    Ok(items)
}

fn push_items(items: &mut Vec<InstallItemRequest>, kind: ItemKind, specs: Vec<ToolSpec>) {
    items.extend(specs.into_iter().map(|spec| InstallItemRequest {
        kind,
        spec: ItemSpec {
            name: spec.name,
            version: spec.version.parse().expect("validated ToolSpec version"),
            backend: spec.backend,
        },
    }));
}

fn infer_item_kind(spec: &ToolSpec) -> anyhow::Result<ItemKind> {
    let Some(backend) = &spec.backend else {
        anyhow::bail!(
            "cannot infer whether {} is a tool, package, or app; use --tool, --package, or --app",
            spec.name
        );
    };
    match backend.as_str() {
        "rustup" | "mise" | "asdf" | "aqua" | "npm" | "pnpm" | "yarn" | "cargo" | "go" | "pipx" => {
            Ok(ItemKind::Tool)
        }
        "homebrew" | "brew" | "apt" | "dnf" | "pacman" | "nix" => Ok(ItemKind::Package),
        "homebrew-cask" | "brew-cask" | "flatpak" | "snap" | "mas" => Ok(ItemKind::App),
        backend => anyhow::bail!(
            "cannot infer whether {}@{}@{} is a tool, package, or app; use --tool, --package, or --app",
            spec.name,
            spec.version,
            backend
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use anyhow::anyhow;
    use engine::actions::activate::ActivateResult;
    use engine::actions::agents::{AgentsOperation, AgentsResult};
    use engine::actions::config::CheckConfigResult;
    use engine::actions::doctor::DoctorResult;
    use engine::actions::env::EnvResult;
    use engine::actions::init::InitResult;
    use engine::actions::install::InstallResult;
    use engine::actions::list::ListResult;
    use engine::actions::run::RunResult;
    use engine::actions::services::{ServicesOperation, ServicesResult};
    use engine::actions::sync::SyncResult;
    use engine::actions::task::TaskResult;
    use engine::actions::trust::TrustResult;
    use engine::actions::uninstall::UninstallResult;

    use super::*;
    use crate::cli::output::BufferedOutput;

    #[derive(Default)]
    struct FakeRuntime {
        install_result: Option<anyhow::Result<InstallResult>>,
        install_requests: Vec<(ItemKind, String, String, Option<String>)>,
        install_globals: Vec<bool>,
    }

    impl CliRuntime for FakeRuntime {
        fn install(&mut self, request: InstallCommandRequest) -> anyhow::Result<InstallResult> {
            self.install_globals.push(request.global);
            self.install_requests
                .extend(request.install.items.into_iter().map(|item| {
                    (
                        item.kind,
                        item.spec.name,
                        item.spec.version.to_string(),
                        item.spec.backend.map(|backend| backend.to_string()),
                    )
                }));
            self.install_result
                .take()
                .expect("test runtime install result was not configured")
        }

        fn config_check(&mut self, _global: bool) -> anyhow::Result<CheckConfigResult> {
            panic!("config_check should not run in install tests");
        }

        fn init(&mut self, _force: bool) -> anyhow::Result<InitResult> {
            panic!("init should not run in install tests");
        }

        fn env(&mut self, _global: bool) -> anyhow::Result<EnvResult> {
            panic!("env should not run in install tests");
        }

        fn list(&mut self, _all: bool) -> anyhow::Result<ListResult> {
            panic!("list should not run in install tests");
        }

        fn agents(&mut self, _operation: AgentsOperation) -> anyhow::Result<AgentsResult> {
            panic!("agents should not run in install tests");
        }

        fn run_command(&mut self, _command: Vec<String>) -> anyhow::Result<RunResult> {
            panic!("run_command should not run in install tests");
        }

        fn task(&mut self, _name: Option<String>) -> anyhow::Result<TaskResult> {
            panic!("task should not run in install tests");
        }

        fn activate(&mut self, _shell: Option<String>) -> anyhow::Result<ActivateResult> {
            panic!("activate should not run in install tests");
        }

        fn doctor(&mut self) -> anyhow::Result<DoctorResult> {
            panic!("doctor should not run in install tests");
        }

        fn sync(&mut self) -> anyhow::Result<SyncResult> {
            panic!("sync should not run in install tests");
        }

        fn services(
            &mut self,
            _operation: ServicesOperation,
            _name: Option<String>,
        ) -> anyhow::Result<ServicesResult> {
            panic!("services should not run in install tests");
        }

        fn trust(&mut self) -> anyhow::Result<TrustResult> {
            panic!("trust should not run in install tests");
        }

        fn uninstall(&mut self, _name: String) -> anyhow::Result<UninstallResult> {
            panic!("uninstall should not run in install tests");
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
            vec![(
                ItemKind::Tool,
                "ripgrep".to_string(),
                "latest".to_string(),
                None
            )]
        );
        assert_eq!(runtime.install_globals, [false]);
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

    #[test]
    fn install_passes_global_scope_to_runtime() {
        let mut args = install_args("ripgrep");
        args.global = true;
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
        assert_eq!(runtime.install_globals, [true]);
    }

    #[test]
    fn install_infers_unclassified_items_from_backend() {
        let mut args = install_args("jq");
        args.tools.clear();
        args.items = vec![
            "rust@stable@rustup".parse().unwrap(),
            "openssl@latest@homebrew".parse().unwrap(),
            "firefox@latest@homebrew-cask".parse().unwrap(),
        ];
        let mut runtime = FakeRuntime {
            install_result: Some(Ok(InstallResult {
                tool_name: "firefox".to_string(),
                version: "latest".to_string(),
                install_path: PathBuf::from("/opt/still/apps/firefox/latest"),
                binary_path: None,
            })),
            ..FakeRuntime::default()
        };
        let mut output = BufferedOutput::default();

        let code = run(args, &mut runtime, &mut output);

        assert_eq!(code, 0);
        assert_eq!(
            runtime.install_requests,
            [
                (
                    ItemKind::Tool,
                    "rust".to_string(),
                    "stable".to_string(),
                    Some("rustup".to_string())
                ),
                (
                    ItemKind::Package,
                    "openssl".to_string(),
                    "latest".to_string(),
                    Some("homebrew".to_string())
                ),
                (
                    ItemKind::App,
                    "firefox".to_string(),
                    "latest".to_string(),
                    Some("homebrew-cask".to_string())
                )
            ]
        );
    }

    #[test]
    fn install_errors_when_unclassified_item_is_ambiguous() {
        let mut args = install_args("jq");
        args.tools.clear();
        args.items = vec!["jq".parse().unwrap()];
        let mut runtime = FakeRuntime::default();
        let mut output = BufferedOutput::default();

        let code = run(args, &mut runtime, &mut output);

        assert_eq!(code, 1);
        assert!(runtime.install_requests.is_empty());
        insta::assert_snapshot!(output.stderr, @r###"
install failed: cannot infer whether jq is a tool, package, or app; use --tool, --package, or --app
"###);
    }

    fn install_args(tool: &str) -> InstallArgs {
        InstallArgs {
            global: false,
            tools: vec![tool.parse().expect("test tool spec should parse")],
            packages: Vec::new(),
            apps: Vec::new(),
            items: Vec::new(),
        }
    }
}
