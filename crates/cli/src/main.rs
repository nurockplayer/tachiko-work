mod commands;

use std::{path::PathBuf, process::ExitCode};

use clap::{Parser, Subcommand, ValueEnum};
use tachiko_workflow::StarterTemplate;

#[derive(Debug, Parser)]
#[command(name = "tachiko", version, about = "Semantic computational documents")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Create a semantic document with a useful game-balance starter
    Init {
        path: PathBuf,
        #[arg(long)]
        id: Option<String>,
        #[arg(long)]
        title: Option<String>,
        /// Starting content (game-balance is immediately explorable)
        #[arg(long, value_enum, default_value_t = TemplateName::GameBalance)]
        template: TemplateName,
    },
    /// Check structure, references, types, and formulas
    Validate { path: PathBuf },
    /// Materialize all numeric and formula values as JSON
    Calculate { path: PathBuf },
    /// Browse entities and calculated values
    Show { path: PathBuf },
    /// Explain a field, its formula, and downstream impact
    Explain {
        path: PathBuf,
        /// Field identifier in entity.field form
        field: String,
    },
    /// Create a changed document from one schema-typed field edit
    Set {
        input: PathBuf,
        /// Field identifier in entity.field form
        field: String,
        /// New number, text, boolean, or referenced entity id
        value: String,
        /// New .ro document to create; existing files are never overwritten
        #[arg(long)]
        output: PathBuf,
    },
    /// Compare two document versions by semantic meaning
    Diff { before: PathBuf, after: PathBuf },
    /// Merge semantic changes from two document versions
    Merge {
        base: PathBuf,
        ours: PathBuf,
        theirs: PathBuf,
        /// New .ro document to create; existing files are never overwritten
        #[arg(long)]
        output: PathBuf,
    },
    /// Export a calculated runtime JSON projection
    Export { input: PathBuf, output: PathBuf },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum TemplateName {
    GameBalance,
    Empty,
}

impl From<TemplateName> for StarterTemplate {
    fn from(value: TemplateName) -> Self {
        match value {
            TemplateName::GameBalance => Self::GameBalance,
            TemplateName::Empty => Self::Empty,
        }
    }
}

fn main() -> ExitCode {
    match execute(Cli::parse()) {
        Ok(output) => {
            print!("{output}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn execute(cli: Cli) -> Result<String, commands::CommandError> {
    match cli.command {
        Commands::Init {
            path,
            id,
            title,
            template,
        } => commands::init(&path, id, title, template.into()),
        Commands::Validate { path } => commands::validate(&path),
        Commands::Calculate { path } => commands::calculate_document(&path),
        Commands::Show { path } => commands::show(&path),
        Commands::Explain { path, field } => commands::explain(&path, &field),
        Commands::Set {
            input,
            field,
            value,
            output,
        } => commands::set_document(&input, &field, &value, &output),
        Commands::Diff { before, after } => commands::diff_documents(&before, &after),
        Commands::Merge {
            base,
            ours,
            theirs,
            output,
        } => commands::merge_documents(&base, &ours, &theirs, &output),
        Commands::Export { input, output } => commands::export(&input, &output),
    }
}
