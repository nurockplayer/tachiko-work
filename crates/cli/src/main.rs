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
        /// Explicit stable document identity (advanced; normally UUIDv7-generated)
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
    /// Grow, rename, or remove entities safely
    Entity {
        #[command(subcommand)]
        command: EntityCommands,
    },
    /// Create or revise computed fields safely
    Formula {
        #[command(subcommand)]
        command: FormulaCommands,
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

#[derive(Debug, Subcommand)]
enum EntityCommands {
    /// Duplicate an entity and rebase its self-referential formulas
    Duplicate {
        input: PathBuf,
        /// Existing entity identifier to copy
        source: String,
        /// New entity identifier to create
        target: String,
        /// New .ro document to create; existing files are never overwritten
        #[arg(long)]
        output: PathBuf,
    },
    /// Rename an entity key while preserving stable identity and references
    Rename {
        input: PathBuf,
        /// Existing entity identifier
        source: String,
        /// New entity identifier
        target: String,
        /// New .ro document to create; existing files are never overwritten
        #[arg(long)]
        output: PathBuf,
    },
    /// Remove an unreferenced entity
    Remove {
        input: PathBuf,
        /// Entity identifier to remove
        entity: String,
        /// New .ro document to create; existing files are never overwritten
        #[arg(long)]
        output: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
enum FormulaCommands {
    /// Set a numeric field formula
    Set {
        input: PathBuf,
        /// Field identifier in entity.field form
        field: String,
        /// Formula expression; quote it in a shell
        #[arg(long, allow_hyphen_values = true)]
        expression: String,
        /// New .ro document to create; existing files are never overwritten
        #[arg(long)]
        output: PathBuf,
    },
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
        Commands::Entity { command } => match command {
            EntityCommands::Duplicate {
                input,
                source,
                target,
                output,
            } => commands::duplicate_entity_document(&input, &source, &target, &output),
            EntityCommands::Rename {
                input,
                source,
                target,
                output,
            } => commands::rename_entity_document(&input, &source, &target, &output),
            EntityCommands::Remove {
                input,
                entity,
                output,
            } => commands::remove_entity_document(&input, &entity, &output),
        },
        Commands::Formula { command } => match command {
            FormulaCommands::Set {
                input,
                field,
                expression,
                output,
            } => commands::set_formula_document(&input, &field, &expression, &output),
        },
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
