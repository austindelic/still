use std::str::FromStr;

use clap::{
    Arg, ArgAction, ArgMatches, Args, FromArgMatches, Parser, Subcommand, error::ErrorKind,
};
use engine::specs::item::{BackendId, ItemKind, ItemSpec};

#[derive(Parser, Debug)]
#[command(
    name = "Still",
    about = "Universal Package Manager + Version Manager",
    long_about = None
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    #[command(about = "Initialize configuration for a new project")]
    Init(InitArgs),
    #[command(about = "Trust project-defined executable behavior")]
    Trust(TrustArgs),
    #[command(about = "Install requested tools, packages, or apps")]
    Install(InstallArgs),
    #[command(about = "Synchronize installed state with config")]
    Sync(SyncArgs),
    #[command(about = "List configured and installed items")]
    List(ListArgs),
    #[command(about = "Remove a Still-managed install")]
    Uninstall(UninstallArgs),
    #[command(about = "Run a command within the managed environment")]
    Run(RunArgs),
    #[command(about = "Run or list tasks defined in config")]
    Task(TaskArgs),
    #[command(about = "Inspect, start, stop, or check services")]
    Services(ServicesArgs),
    #[command(about = "Inspect, sync, or validate agent instructions and skills")]
    Agents(AgentsArgs),
    #[command(about = "Inspect, validate, or edit Still configuration")]
    Config(ConfigArgs),
    #[command(about = "Diagnose machine, cache, config, and platform health")]
    Doctor(DoctorArgs),
    #[command(about = "Display resolved environment information")]
    Env(EnvArgs),
    #[command(about = "Print shell activation code")]
    Activate(ActivateArgs),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallArgs {
    pub global: bool,
    pub force: bool,
    pub items: Vec<InstallItemArg>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallItemArg {
    pub kind: Option<ItemKind>,
    pub spec: ItemSpec,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InstallEvent {
    Kind(ItemKind),
    Source(SourceFlag),
    Spec,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourceFlag {
    Homebrew,
    Cargo,
    Npm,
    Pipx,
    Go,
    Aqua,
    Apt,
    Dnf,
    Pacman,
    Winget,
    Flatpak,
}

impl SourceFlag {
    fn as_str(self) -> &'static str {
        match self {
            Self::Homebrew => "homebrew",
            Self::Cargo => "cargo",
            Self::Npm => "npm",
            Self::Pipx => "pipx",
            Self::Go => "go",
            Self::Aqua => "aqua",
            Self::Apt => "apt",
            Self::Dnf => "dnf",
            Self::Pacman => "pacman",
            Self::Winget => "winget",
            Self::Flatpak => "flatpak",
        }
    }
}

impl Args for InstallArgs {
    fn augment_args(cmd: clap::Command) -> clap::Command {
        cmd.arg(
            Arg::new("global")
                .short('g')
                .long("global")
                .help("Record the requested items in the global Still config")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("force")
                .short('f')
                .long("force")
                .help(
                    "Update an existing config entry when the requested version or source differs",
                )
                .action(ArgAction::SetTrue),
        )
        .arg(kind_flag(
            "tool",
            Some('t'),
            "Classify following specs as tools",
        ))
        .arg(kind_flag(
            "package",
            Some('p'),
            "Classify following specs as packages",
        ))
        .arg(kind_flag(
            "app",
            Some('a'),
            "Classify following specs as apps",
        ))
        .arg(source_flag(
            "homebrew",
            "Select Homebrew for following specs",
        ))
        .arg(
            source_flag("cargo", "Select Cargo for following specs")
                .long_help("Select Cargo for following specs, such as ripgrep@14.1.1"),
        )
        .arg(source_flag("npm", "Select npm for following specs"))
        .arg(source_flag("pipx", "Select pipx for following specs"))
        .arg(source_flag("go", "Select Go for following specs"))
        .arg(source_flag("aqua", "Select Aqua for following specs"))
        .arg(source_flag("apt", "Select apt for following specs"))
        .arg(source_flag("dnf", "Select dnf for following specs"))
        .arg(source_flag("pacman", "Select pacman for following specs"))
        .arg(source_flag("winget", "Select winget for following specs"))
        .arg(source_flag("flatpak", "Select Flatpak for following specs"))
        .arg(
            Arg::new("spec")
                .value_name("SPEC")
                .help("Item specs: name, name@version, source:name, or source:name@version")
                .num_args(0..)
                .action(ArgAction::Append),
        )
    }

    fn augment_args_for_update(cmd: clap::Command) -> clap::Command {
        Self::augment_args(cmd)
    }
}

impl FromArgMatches for InstallArgs {
    fn from_arg_matches(matches: &ArgMatches) -> Result<Self, clap::Error> {
        let global = matches.get_flag("global");
        let force = matches.get_flag("force");
        let specs = matches
            .get_many::<String>("spec")
            .map(|values| values.cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        let spec_indices = matches
            .indices_of("spec")
            .map(|values| values.collect::<Vec<_>>())
            .unwrap_or_default();

        let mut spec_iter = spec_indices.into_iter().zip(specs);
        let mut events = Vec::new();
        collect_flag_events(
            matches,
            &mut events,
            "tool",
            InstallEvent::Kind(ItemKind::Tool),
        );
        collect_flag_events(
            matches,
            &mut events,
            "package",
            InstallEvent::Kind(ItemKind::Package),
        );
        collect_flag_events(
            matches,
            &mut events,
            "app",
            InstallEvent::Kind(ItemKind::App),
        );
        for (id, source) in [
            ("homebrew", SourceFlag::Homebrew),
            ("cargo", SourceFlag::Cargo),
            ("npm", SourceFlag::Npm),
            ("pipx", SourceFlag::Pipx),
            ("go", SourceFlag::Go),
            ("aqua", SourceFlag::Aqua),
            ("apt", SourceFlag::Apt),
            ("dnf", SourceFlag::Dnf),
            ("pacman", SourceFlag::Pacman),
            ("winget", SourceFlag::Winget),
            ("flatpak", SourceFlag::Flatpak),
        ] {
            collect_flag_events(matches, &mut events, id, InstallEvent::Source(source));
        }
        for (index, _) in spec_iter.clone() {
            events.push((index, InstallEvent::Spec));
        }
        events.sort_by_key(|(index, _)| *index);

        let mut active_kind = None;
        let mut active_source = None;
        let mut items = Vec::new();
        for (_, event) in events {
            match event {
                InstallEvent::Kind(kind) => active_kind = Some(kind),
                InstallEvent::Source(source) => active_source = Some(source),
                InstallEvent::Spec => {
                    let (_, raw) = spec_iter.next().expect("spec event should have a value");
                    let spec = parse_install_spec(&raw, active_source)?;
                    items.push(InstallItemArg {
                        kind: active_kind,
                        spec,
                    });
                }
            }
        }

        if items.is_empty() {
            return Err(clap::Error::raw(
                ErrorKind::MissingRequiredArgument,
                "install requires at least one item spec",
            ));
        }

        Ok(Self {
            global,
            force,
            items,
        })
    }

    fn update_from_arg_matches(&mut self, matches: &ArgMatches) -> Result<(), clap::Error> {
        *self = Self::from_arg_matches(matches)?;
        Ok(())
    }
}

fn kind_flag(id: &'static str, short: Option<char>, help: &'static str) -> Arg {
    let arg = Arg::new(id).long(id).help(help).action(ArgAction::Count);
    if let Some(short) = short {
        arg.short(short)
    } else {
        arg
    }
}

fn source_flag(id: &'static str, help: &'static str) -> Arg {
    Arg::new(id).long(id).help(help).action(ArgAction::Count)
}

fn collect_flag_events(
    matches: &ArgMatches,
    events: &mut Vec<(usize, InstallEvent)>,
    id: &str,
    event: InstallEvent,
) {
    if let Some(indices) = matches.indices_of(id) {
        events.extend(indices.map(|index| (index, event)));
    }
}

fn parse_install_spec(
    raw: &str,
    active_source: Option<SourceFlag>,
) -> Result<ItemSpec, clap::Error> {
    let (source, body) = match raw.split_once(':') {
        Some((source, body)) => (Some(parse_source_id(raw, source)?), body),
        None => (None, raw),
    };

    let mut spec = parse_name_version(body)?;
    if spec.backend.is_none() {
        spec.backend = source.or_else(|| {
            active_source
                .map(SourceFlag::as_str)
                .map(BackendId::new)
                .transpose()
                .expect("built-in source flags are valid")
        });
    }
    Ok(spec)
}

fn parse_source_id(raw: &str, source: &str) -> Result<BackendId, clap::Error> {
    BackendId::new(source).map_err(|err| invalid_spec(raw, err))
}

fn parse_name_version(raw: &str) -> Result<ItemSpec, clap::Error> {
    let parts = raw.split('@').collect::<Vec<_>>();
    if parts.len() > 2 {
        return Err(invalid_spec(
            raw,
            "expected name, name@version, source:name, or source:name@version",
        ));
    }
    let name = parts[0];
    let version = parts.get(1).copied().unwrap_or("latest");
    let spec = format!("{name}@{version}");
    spec.parse::<ItemSpec>()
        .map_err(|err| invalid_spec(raw, err))
}

fn invalid_spec(raw: &str, err: impl std::fmt::Display) -> clap::Error {
    clap::Error::raw(
        ErrorKind::ValueValidation,
        format!("invalid install spec '{raw}': {err}"),
    )
}

#[derive(clap::Args, Debug, Clone)]
#[command(group(
    clap::ArgGroup::new("target")
        .required(true)
        .multiple(false)
        .args(["tool", "package", "app", "item"])
))]
pub struct UninstallArgs {
    #[arg(short = 'g', long)]
    pub global: bool,
    #[arg(short = 't', long = "tool", value_name = "TOOL")]
    pub tool: Option<UninstallSpec>,
    #[arg(short = 'p', long = "package", value_name = "PACKAGE")]
    pub package: Option<UninstallSpec>,
    #[arg(short = 'a', long = "app", value_name = "APP")]
    pub app: Option<UninstallSpec>,
    #[arg(value_name = "ITEM@VERSION")]
    pub item: Option<UninstallSpec>,
}

impl UninstallArgs {
    pub fn target(&self) -> Option<(Option<ItemKind>, &UninstallSpec)> {
        self.tool
            .as_ref()
            .map(|spec| (Some(ItemKind::Tool), spec))
            .or_else(|| {
                self.package
                    .as_ref()
                    .map(|spec| (Some(ItemKind::Package), spec))
            })
            .or_else(|| self.app.as_ref().map(|spec| (Some(ItemKind::App), spec)))
            .or_else(|| self.item.as_ref().map(|spec| (None, spec)))
    }
}

#[derive(Debug, Clone)]
pub struct UninstallSpec {
    pub spec: ItemSpec,
    pub exact: bool,
}

impl FromStr for UninstallSpec {
    type Err = engine::error::EngineError;

    fn from_str(input: &str) -> engine::error::Result<Self> {
        Ok(Self {
            spec: input.parse()?,
            exact: input.contains('@'),
        })
    }
}

#[derive(clap::Args, Debug, Clone)]
pub struct TrustArgs {}

#[derive(clap::Args, Debug, Clone)]
pub struct SyncArgs {
    #[arg(short, long)]
    pub global: bool,
}

#[derive(clap::Args, Debug, Clone)]
pub struct ListArgs {
    #[arg(short, long)]
    pub global: bool,
    #[arg(long)]
    pub all: bool,
}

#[derive(clap::Args, Debug, Clone)]
pub struct DoctorArgs {}

#[derive(clap::Args, Debug, Clone)]
pub struct RunArgs {
    #[arg(short, long)]
    pub global: bool,
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, required = true)]
    pub command: Vec<String>,
}

#[derive(clap::Args, Debug, Clone)]
pub struct TaskArgs {
    #[arg(short, long)]
    pub global: bool,
    pub name: Option<String>,
}

#[derive(clap::Args, Debug, Clone)]
pub struct ServicesArgs {
    #[arg(short, long)]
    pub global: bool,
    #[command(subcommand)]
    pub command: Option<ServicesCommand>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum ServicesCommand {
    Status { name: Option<String> },
    Start { name: Option<String> },
    Stop { name: Option<String> },
    Check { name: Option<String> },
}

#[derive(clap::Args, Debug, Clone)]
pub struct AgentsArgs {
    #[arg(short, long)]
    pub global: bool,
    #[command(subcommand)]
    pub command: Option<AgentsCommand>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum AgentsCommand {
    List,
    Sync {
        #[arg(long)]
        accept_auto_deps: bool,
    },
    Check,
}

#[derive(clap::Args, Debug, Clone)]
pub struct InitArgs {
    #[arg(long)]
    pub force: bool,
}

#[derive(clap::Args, Debug, Clone)]
pub struct ConfigArgs {
    #[arg(short, long)]
    pub global: bool,
    #[command(subcommand)]
    pub command: ConfigCommand,
}

#[derive(Subcommand, Debug, Clone)]
pub enum ConfigCommand {
    Check,
}

#[derive(clap::Args, Debug, Clone)]
pub struct EnvArgs {
    #[arg(short, long)]
    pub global: bool,
}

#[derive(clap::Args, Debug, Clone)]
pub struct ActivateArgs {
    #[arg(short, long)]
    pub global: bool,
    #[arg(long)]
    pub shell: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    #[test]
    fn parses_explicit_source_spec() {
        let cli = Cli::try_parse_from(["still", "install", "--tool", "cargo:ripgrep@14.1.1"])
            .expect("source syntax should parse");
        let Some(Command::Install(args)) = cli.command else {
            panic!("expected install command");
        };

        assert_eq!(args.items[0].kind, Some(ItemKind::Tool));
        assert_eq!(args.items[0].spec.name, "ripgrep");
        assert_eq!(args.items[0].spec.version.as_str(), "14.1.1");
        assert_eq!(
            args.items[0].spec.backend.as_ref().unwrap().as_str(),
            "cargo"
        );
    }

    #[test]
    fn source_flags_apply_until_changed_without_changing_kind() {
        let cli = Cli::try_parse_from([
            "still",
            "install",
            "--tool",
            "--cargo",
            "ripgrep@14.1.1",
            "cargo-nextest",
            "--npm",
            "typescript@5.8.0",
            "--package",
            "--apt",
            "openssl",
        ])
        .expect("ordered source flags should parse");
        let Some(Command::Install(args)) = cli.command else {
            panic!("expected install command");
        };

        assert_eq!(args.items[0].kind, Some(ItemKind::Tool));
        assert_eq!(
            args.items[0].spec.backend.as_ref().unwrap().as_str(),
            "cargo"
        );
        assert_eq!(args.items[1].kind, Some(ItemKind::Tool));
        assert_eq!(
            args.items[1].spec.backend.as_ref().unwrap().as_str(),
            "cargo"
        );
        assert_eq!(args.items[2].kind, Some(ItemKind::Tool));
        assert_eq!(args.items[2].spec.backend.as_ref().unwrap().as_str(), "npm");
        assert_eq!(args.items[3].kind, Some(ItemKind::Package));
        assert_eq!(args.items[3].spec.backend.as_ref().unwrap().as_str(), "apt");
    }

    #[test]
    fn explicit_source_overrides_active_source_flag() {
        let cli = Cli::try_parse_from([
            "still",
            "install",
            "--tool",
            "--cargo",
            "aqua:ripgrep@14.1.1",
        ])
        .expect("explicit source should parse");
        let Some(Command::Install(args)) = cli.command else {
            panic!("expected install command");
        };

        assert_eq!(
            args.items[0].spec.backend.as_ref().unwrap().as_str(),
            "aqua"
        );
    }

    #[test]
    fn empty_versions_normalize_to_latest() {
        let cli = Cli::try_parse_from(["still", "install", "--tool", "cargo:ripgrep@"])
            .expect("empty version should parse as latest");
        let Some(Command::Install(args)) = cli.command else {
            panic!("expected install command");
        };

        assert_eq!(args.items[0].spec.version.as_str(), "latest");
    }

    #[test]
    fn install_requires_at_least_one_spec() {
        let err = Cli::try_parse_from(["still", "install", "--tool"]).unwrap_err();

        assert!(
            err.to_string()
                .contains("install requires at least one item spec")
        );
    }

    #[test]
    fn run_accepts_child_flags_after_command() {
        let cli = Cli::try_parse_from(["still", "run", "cargo", "test", "--", "--quiet"])
            .expect("run args should parse child flags");
        let Some(Command::Run(args)) = cli.command else {
            panic!("expected run command");
        };

        assert_eq!(args.command, ["cargo", "test", "--", "--quiet"]);
    }

    impl Cli {
        fn try_parse_from<I, T>(itr: I) -> Result<Self, clap::Error>
        where
            I: IntoIterator<Item = T>,
            T: Into<OsString> + Clone,
        {
            <Self as Parser>::try_parse_from(itr)
        }
    }
}
