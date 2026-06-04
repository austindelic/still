use engine::{
    actions::install::{InstallItemRequest, InstallRequest, InstallResult, ToolInstallOptions},
    error::{EngineError, Result},
    resolve::infer_item_kind_from_backend,
    specs::item::{ItemKind, ItemSpec},
};

use crate::cli::{
    args::{InstallArgs, InstallItemArg},
    output::Output,
    progress::Spinner,
    runtime::{CliRuntime, ScopedInstallRequest},
};

pub fn run<R, O>(args: InstallArgs, runtime: &mut R, output: &mut O) -> i32
where
    R: CliRuntime,
    O: Output,
{
    let global = args.global;
    let force = args.force;
    let items = match install_items(args.items) {
        Ok(items) => items,
        Err(e) => {
            output.error(&format!("install failed: {e}"));
            return 1;
        }
    };
    let request_items = items.clone();
    let install_request = ScopedInstallRequest {
        global,
        force,
        install: InstallRequest { items },
    };

    let spinner = Spinner::start("Installing requested items");
    let result = runtime.install(install_request);
    spinner.finish();

    match result {
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

fn install_items(args: Vec<InstallItemArg>) -> Result<Vec<InstallItemRequest>> {
    args.into_iter()
        .map(|item| {
            let kind = match item.kind {
                Some(kind) => kind,
                None => infer_kind(&item.spec)?,
            };
            Ok(InstallItemRequest {
                kind,
                spec: item.spec,
                tool: ToolInstallOptions::default(),
            })
        })
        .collect()
}

fn infer_kind(spec: &ItemSpec) -> Result<ItemKind> {
    let Some(source) = &spec.backend else {
        return Err(EngineError::message(format!(
            "cannot infer whether {} is a tool, package, or app; use --tool, --package, or --app",
            spec.name
        )));
    };
    infer_item_kind_from_backend(source.as_str()).ok_or_else(|| {
        EngineError::message(format!(
            "cannot infer whether {}@{} from {} is a tool, package, or app; use --tool, --package, or --app",
            spec.name, spec.version, source
        ))
    })
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
