use clap::{Args as _, Command, FromArgMatches as _};
use std::env;
use std::io;
use std::path::Path;
use std::process::ExitCode;

use rip2::args::RipCommands;
use rip2::safety::SystemRootChecker;
use rip2::{args, completions, detect_mode_from_inputs, util, ExecutionMode};

/// Detect execution mode from actual environment and process arguments
fn detect_mode() -> ExecutionMode {
    // Get RIP_MODE environment variable
    let env_mode = env::var("RIP_MODE").ok();

    // Get binary name from argv[0]
    let binary_name = env::args_os()
        .next()
        .and_then(|argv0| {
            Path::new(&argv0)
                .file_name()
                .map(|s| s.to_os_string())
        })
        .unwrap_or_default();

    detect_mode_from_inputs(env_mode.as_deref(), &binary_name)
}

fn main() -> ExitCode {
    // Detect execution mode from env/binary name
    let exec_mode = detect_mode();

    match exec_mode {
        ExecutionMode::Rip => run_rip_mode(),
        ExecutionMode::Rm => run_rm_mode(),
    }
}

/// Run in rip mode: ergonomic interface with subcommands
fn run_rip_mode() -> ExitCode {
    let base_cmd = Command::new("rip");
    let cmd = args::RipArgs::augment_args(base_cmd);
    let cli = args::RipArgs::from_arg_matches(&cmd.get_matches()).unwrap();

    match &cli.command {
        Some(RipCommands::Completions { shell }) => {
            let result = completions::generate_shell_completions(shell, &mut io::stdout());
            if result.is_err() {
                eprintln!("{}", result.unwrap_err());
                return ExitCode::FAILURE;
            }
        }
        Some(RipCommands::Graveyard { seance }) => {
            let graveyard = rip2::get_graveyard(None);
            if *seance {
                let cwd = env::current_dir().expect("Failed to get current directory");
                let gravepath = util::join_absolute(
                    graveyard,
                    dunce::canonicalize(cwd).expect("Failed to get current directory"),
                );
                print!("{}", gravepath.display());
            } else {
                print!("{}", graveyard.display());
            }
        }
        None => {
            let mut stream = io::stdout();
            let mode = util::ProductionMode;

            let result = rip2::run(&cli, mode, &mut stream, &SystemRootChecker);

            if let Err(ref e) = result {
                println!("Exception: {e}");
                return ExitCode::FAILURE;
            }
        }
    }

    ExitCode::SUCCESS
}

/// Run in rm mode: POSIX-compatible interface
///
/// Parses RmArgs with rm-compatible flags (-r, -f, -d, -v, etc.)
/// Files are moved to graveyard instead of permanent deletion.
fn run_rm_mode() -> ExitCode {
    // Parse rm-style arguments
    let base_cmd = Command::new("rm");
    let cmd = args::RmArgs::augment_args(base_cmd);
    let cli = args::RmArgs::from_arg_matches(&cmd.get_matches()).unwrap();

    // Validate arguments
    if let Err(e) = args::validate_rm_args(&cli) {
        eprintln!("rm: {e}");
        return ExitCode::FAILURE;
    }

    // TODO(Phase 4): Implement rm-specific behavior
    // For now, convert RmArgs to RipArgs-compatible behavior
    // This is a temporary bridge until full rm mode implementation

    if cli.targets.is_empty() {
        eprintln!("rm: missing operand");
        eprintln!("Try 'rm --help' for more information.");
        return ExitCode::FAILURE;
    }

    // Create equivalent RipArgs for the bridge period
    let rip_cli = args::RipArgs {
        targets: cli.targets.clone(),
        graveyard: None,
        decompose: false,
        seance: false,
        unbury: None,
        inspect: false,
        force: cli.force,
        command: None,
    };

    // Execute using rip mode logic
    let mut stream = io::stdout();
    let mode = util::ProductionMode;
    let result = rip2::run(&rip_cli, mode, &mut stream, &rip2::safety::SystemRootChecker);

    if let Err(ref e) = result {
        // Use rm-style error format
        eprintln!("rm: {e}");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}
