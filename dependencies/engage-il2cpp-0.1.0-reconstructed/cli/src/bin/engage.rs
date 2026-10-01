use anyhow::Result;
use clap::{Parser, Subcommand};

use engage::commands;

#[derive(Parser)]
#[command(name = "engage", version, about = "Manage engage-il2cpp Cargo features for your project")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Features {
        #[command(subcommand)]
        action: Option<FeaturesAction>,
    },
}

#[derive(Subcommand)]
enum FeaturesAction {
    Check,
    Apply {
        #[arg(long)]
        yes: bool,
    },
    Add {
        name: String,
    },
    Explain {
        path: String,
    },
    Prune {
        #[arg(long)]
        yes: bool,
    },
    /// Resolve the methods-feature for a receiver path. Used by the VS Code
    /// extension to turn `E0599 no method named X found for struct Y` into
    /// an "enable feature for Y" quick-fix.
    ForReceiver {
        receiver: String,
    },
    /// List every engage-il2cpp type whose gating feature is not currently
    /// enabled (transitively) in Cargo.toml. Used by the VS Code extension
    /// to fill in completions for types that rust-analyzer can't see.
    ListDisabled {
        #[arg(long)]
        json: bool,
    },
    /// List every method on a disabled-feature trait/impl block. Used by the
    /// VS Code extension to suggest method names on a `.` after a receiver
    /// whose `IXMethods` trait isn't currently in scope.
    ListDisabledMethods {
        #[arg(long)]
        json: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Features { action } => {
            match action.unwrap_or(FeaturesAction::Check) {
                FeaturesAction::Check => commands::check(),
                FeaturesAction::Apply { yes } => commands::apply(yes),
                FeaturesAction::Add { name } => commands::add_feature(&name),
                FeaturesAction::Explain { path } => commands::explain(&path),
                FeaturesAction::Prune { yes } => commands::prune(yes),
                FeaturesAction::ForReceiver { receiver } => commands::for_receiver(&receiver),
                FeaturesAction::ListDisabled { json } => commands::list_disabled(json),
                FeaturesAction::ListDisabledMethods { json } => commands::list_disabled_methods(json),
            }
        },
    }
}
