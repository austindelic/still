//! CLI-facing install command handler.

use crate::cli::args::InstallArgs;
use crate::cli::output::Output;
use crate::cli::runtime::{CliRuntime, InstallCommandRequest};
use engine::actions::install::{InstallItemRequest, InstallRequest, InstallResult};
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
    let force = args.force;
    let items = match install_items(args) {
        Ok(items) => items,
        Err(e) => {
            output.error(&format!("install failed: {e}"));
            return 1;
        }
    };
    let request_items = items.clone();
    let install_request = InstallCommandRequest {
        global,
        force,
        install: InstallRequest { items },
    };

    match runtime.install(install_request) {
        Ok(res) => {
            write_grouped_request(&request_items, output);
            write_install_result(&res, output);
            0
        }
        Err(e) => {
            output.error(&format!("install failed: {e}"));
            1
        }
    }
}

fn write_grouped_request<O: Output>(items: &[InstallItemRequest], output: &mut O) {
    for (kind, heading) in [
        (ItemKind::Tool, "Tools"),
        (ItemKind::Package, "Packages"),
        (ItemKind::App, "Apps"),
    ] {
        let names = items
            .iter()
            .filter(|item| item.kind == kind)
            .map(|item| item.spec.name.as_str())
            .collect::<Vec<_>>();
        if names.is_empty() {
            continue;
        }
        output.info(&format!("{heading}:"));
        for name in names {
            output.info(&format!("  {name}"));
        }
    }
}

fn write_install_result<O: Output>(res: &InstallResult, output: &mut O) {
    if res.installed.is_empty() {
        write_one_install(
            &res.tool_name,
            &res.version,
            &res.install_path,
            res.binary_path.as_deref(),
            output,
        );
        return;
    }

    for item in &res.installed {
        write_one_install(
            &item.name,
            &item.version,
            &item.install_path,
            item.binary_path.as_deref(),
            output,
        );
    }
}

fn write_one_install<O: Output>(
    name: &str,
    version: &str,
    install_path: &std::path::Path,
    binary_path: Option<&std::path::Path>,
    output: &mut O,
) {
    if let Some(binary_path) = binary_path {
        output.info(&format!("Binary installed at: {}", binary_path.display()));
    } else {
        output.warning(&format!(
            "Could not find binary in {}",
            install_path.display()
        ));
    }

    output.success(&format!(
        "Successfully installed {name}@{version} to {}",
        install_path.display()
    ));
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
    use engine::actions::install::{InstallResult, InstalledItemResult};
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
        install_forces: Vec<bool>,
    }

    impl CliRuntime for FakeRuntime {
        fn install(&mut self, request: InstallCommandRequest) -> anyhow::Result<InstallResult> {
            self.install_globals.push(request.global);
            self.install_forces.push(request.force);
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

        fn list(&mut self, _all: bool, _global: bool) -> anyhow::Result<ListResult> {
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

        fn sync(&mut self, _global: bool) -> anyhow::Result<SyncResult> {
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

        fn uninstall(&mut self, _name: String, _global: bool) -> anyhow::Result<UninstallResult> {
            panic!("uninstall should not run in install tests");
        }
    }

    #[test]
    fn install_success_writes_stdout_and_records_request() {
        let args = install_args("ripgrep");
        let mut runtime = FakeRuntime {
            install_result: Some(Ok(install_result(
                ItemKind::Tool,
                "ripgrep",
                "14.1.1",
                "/opt/still/tools/ripgrep/14.1.1",
                Some("/opt/still/tools/ripgrep/14.1.1/bin/rg"),
            ))),
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
        assert_eq!(runtime.install_forces, [false]);
        insta::assert_snapshot!(output.stdout, @r###"
Tools:
  ripgrep
Binary installed at: /opt/still/tools/ripgrep/14.1.1/bin/rg
✓ Successfully installed ripgrep@14.1.1 to /opt/still/tools/ripgrep/14.1.1
"###);
        assert_eq!(output.stderr, "");
    }

    #[test]
    fn install_success_without_binary_writes_warning_to_stderr() {
        let args = install_args("ripgrep");
        let mut runtime = FakeRuntime {
            install_result: Some(Ok(install_result(
                ItemKind::Tool,
                "ripgrep",
                "14.1.1",
                "/opt/still/tools/ripgrep/14.1.1",
                None,
            ))),
            ..FakeRuntime::default()
        };
        let mut output = BufferedOutput::default();

        let code = run(args, &mut runtime, &mut output);

        assert_eq!(code, 0);
        insta::assert_snapshot!(output.stdout, @r###"
Tools:
  ripgrep
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
            install_result: Some(Ok(install_result(
                ItemKind::Tool,
                "ripgrep",
                "14.1.1",
                "/opt/still/tools/ripgrep/14.1.1",
                Some("/opt/still/tools/ripgrep/14.1.1/bin/rg"),
            ))),
            ..FakeRuntime::default()
        };
        let mut output = BufferedOutput::default();

        let code = run(args, &mut runtime, &mut output);

        assert_eq!(code, 0);
        assert_eq!(runtime.install_globals, [true]);
        assert_eq!(runtime.install_forces, [false]);
    }

    #[test]
    fn install_passes_force_to_runtime() {
        let mut args = install_args("ripgrep");
        args.force = true;
        let mut runtime = FakeRuntime {
            install_result: Some(Ok(install_result(
                ItemKind::Tool,
                "ripgrep",
                "14.1.1",
                "/opt/still/tools/ripgrep/14.1.1",
                Some("/opt/still/tools/ripgrep/14.1.1/bin/rg"),
            ))),
            ..FakeRuntime::default()
        };
        let mut output = BufferedOutput::default();

        let code = run(args, &mut runtime, &mut output);

        assert_eq!(code, 0);
        assert_eq!(runtime.install_forces, [true]);
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
            install_result: Some(Ok(install_result(
                ItemKind::App,
                "firefox",
                "latest",
                "/opt/still/apps/firefox/latest",
                None,
            ))),
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
    fn install_reports_grouped_multi_item_request_and_each_result() {
        let mut args = install_args("jq");
        args.tools.push("fd".parse().unwrap());
        args.packages = vec!["openssl".parse().unwrap(), "llvm".parse().unwrap()];
        args.apps = vec!["zed".parse().unwrap(), "firefox".parse().unwrap()];
        let mut result = install_result(
            ItemKind::App,
            "firefox",
            "latest",
            "/opt/still/apps/firefox/latest",
            Some("/opt/still/apps/firefox/latest/firefox"),
        );
        result.installed = vec![
            installed_item(
                ItemKind::Tool,
                "jq",
                "latest",
                "/opt/still/tools/jq/latest",
                Some("/opt/still/tools/jq/latest/bin/jq"),
            ),
            installed_item(
                ItemKind::Tool,
                "fd",
                "latest",
                "/opt/still/tools/fd/latest",
                Some("/opt/still/tools/fd/latest/bin/fd"),
            ),
            installed_item(
                ItemKind::Package,
                "openssl",
                "latest",
                "/opt/still/packages/openssl/latest",
                Some("/opt/still/packages/openssl/latest/bin/openssl"),
            ),
            installed_item(
                ItemKind::Package,
                "llvm",
                "latest",
                "/opt/still/packages/llvm/latest",
                Some("/opt/still/packages/llvm/latest/bin/llvm"),
            ),
            installed_item(
                ItemKind::App,
                "zed",
                "latest",
                "/opt/still/apps/zed/latest",
                Some("/opt/still/apps/zed/latest/zed"),
            ),
            installed_item(
                ItemKind::App,
                "firefox",
                "latest",
                "/opt/still/apps/firefox/latest",
                Some("/opt/still/apps/firefox/latest/firefox"),
            ),
        ];
        let mut runtime = FakeRuntime {
            install_result: Some(Ok(result)),
            ..FakeRuntime::default()
        };
        let mut output = BufferedOutput::default();

        let code = run(args, &mut runtime, &mut output);

        assert_eq!(code, 0);
        insta::assert_snapshot!(output.stdout, @r###"
Tools:
  jq
  fd
Packages:
  openssl
  llvm
Apps:
  zed
  firefox
Binary installed at: /opt/still/tools/jq/latest/bin/jq
✓ Successfully installed jq@latest to /opt/still/tools/jq/latest
Binary installed at: /opt/still/tools/fd/latest/bin/fd
✓ Successfully installed fd@latest to /opt/still/tools/fd/latest
Binary installed at: /opt/still/packages/openssl/latest/bin/openssl
✓ Successfully installed openssl@latest to /opt/still/packages/openssl/latest
Binary installed at: /opt/still/packages/llvm/latest/bin/llvm
✓ Successfully installed llvm@latest to /opt/still/packages/llvm/latest
Binary installed at: /opt/still/apps/zed/latest/zed
✓ Successfully installed zed@latest to /opt/still/apps/zed/latest
Binary installed at: /opt/still/apps/firefox/latest/firefox
✓ Successfully installed firefox@latest to /opt/still/apps/firefox/latest
"###);
        assert_eq!(output.stderr, "");
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
            force: false,
            tools: vec![tool.parse().expect("test tool spec should parse")],
            packages: Vec::new(),
            apps: Vec::new(),
            items: Vec::new(),
        }
    }

    fn install_result(
        kind: ItemKind,
        name: &str,
        version: &str,
        install_path: &str,
        binary_path: Option<&str>,
    ) -> InstallResult {
        InstallResult {
            tool_name: name.to_string(),
            version: version.to_string(),
            install_path: PathBuf::from(install_path),
            binary_path: binary_path.map(PathBuf::from),
            outputs: vec![PathBuf::from(install_path)],
            linked_executables: binary_path.map(PathBuf::from).into_iter().collect(),
            installed: vec![installed_item(
                kind,
                name,
                version,
                install_path,
                binary_path,
            )],
        }
    }

    fn installed_item(
        kind: ItemKind,
        name: &str,
        version: &str,
        install_path: &str,
        binary_path: Option<&str>,
    ) -> InstalledItemResult {
        InstalledItemResult {
            kind,
            name: name.to_string(),
            version: version.to_string(),
            install_path: PathBuf::from(install_path),
            binary_path: binary_path.map(PathBuf::from),
            outputs: vec![PathBuf::from(install_path)],
            linked_executables: binary_path.map(PathBuf::from).into_iter().collect(),
        }
    }
}
