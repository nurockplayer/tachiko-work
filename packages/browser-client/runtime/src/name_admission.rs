//! Command-failure evidence before native naming creates semantic subjects.
use super::{DesignerError, FieldId, NewTableColumnInput, Schema, table_error};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use tachiko_workspace_engine::{SemanticSubject, display_name_failures, trim_display_name};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NameInput {
    Table,
    Column {
        index: usize,
        schema: Option<String>,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct NameFailure {
    pub reason: String,
    pub inputs: Vec<NameInput>,
    pub subjects: Vec<SemanticSubject>,
    pub effective_name: Option<String>,
}

pub(super) fn check_name<'a>(
    input: &'a str,
    locator: Option<&NameInput>,
    subjects: &[SemanticSubject],
    failures: &mut Vec<NameFailure>,
) -> Option<&'a str> {
    let name = trim_display_name(input);
    let spelling = display_name_failures(name);
    for failure in &spelling {
        failures.push(NameFailure {
            reason: failure.reason().to_owned(),
            inputs: locator.cloned().into_iter().collect(),
            subjects: subjects.to_owned(),
            effective_name: None,
        });
    }
    spelling.is_empty().then_some(name)
}

pub(super) fn refuse(failures: Vec<NameFailure>) -> Result<(), DesignerError> {
    if failures.is_empty() {
        Ok(())
    } else {
        Err(DesignerError::NameAdmission { failures })
    }
}

pub(super) fn author_column_name(
    input: &str,
    schema: &Schema,
    field: Option<&FieldId>,
) -> Result<String, DesignerError> {
    let mut failures = Vec::new();
    let subjects = field
        .map(|field| SemanticSubject::SchemaField {
            schema: schema.id.clone(),
            field: field.clone(),
        })
        .into_iter()
        .collect::<Vec<_>>();
    let locator = field.is_none().then(|| NameInput::Column {
        index: 0,
        schema: Some(schema.id.to_string()),
    });
    let name = check_name(input, locator.as_ref(), &subjects, &mut failures);
    if let Some(name) = name.filter(|_| field.is_none()) {
        let mut collisions = schema
            .fields
            .values()
            .filter(|existing| Some(&existing.id) != field && existing.effective_name() == name)
            .map(|existing| SemanticSubject::SchemaField {
                schema: schema.id.clone(),
                field: existing.id.clone(),
            })
            .collect::<Vec<_>>();
        if !collisions.is_empty() {
            if let Some(field) = field {
                collisions.push(SemanticSubject::SchemaField {
                    schema: schema.id.clone(),
                    field: field.clone(),
                });
            }
            collisions.sort();
            failures.push(NameFailure {
                reason: "duplicate_display_name".to_owned(),
                inputs: locator.into_iter().collect(),
                subjects: collisions,
                effective_name: Some(name.to_owned()),
            });
        }
    }
    refuse(failures)?;
    Ok(name.expect("spelling admitted").to_owned())
}

pub(super) fn author_table_names<'a>(
    input: &'a str,
    columns: &'a [NewTableColumnInput],
) -> Result<(&'a str, Vec<&'a str>), DesignerError> {
    let mut failures = Vec::new();
    let name = check_name(input, Some(&NameInput::Table), &[], &mut failures);
    let mut groups = BTreeMap::<&str, Vec<usize>>::new();
    let mut names = Vec::new();
    for (index, column) in columns.iter().enumerate() {
        if let Some(name) = check_name(
            &column.name,
            Some(&NameInput::Column {
                index,
                schema: None,
            }),
            &[],
            &mut failures,
        ) {
            groups.entry(name).or_default().push(index);
            names.push(name);
        }
    }
    for (name, indices) in groups {
        if indices.len() > 1 {
            failures.push(NameFailure {
                reason: "duplicate_display_name".to_owned(),
                inputs: indices
                    .into_iter()
                    .map(|index| NameInput::Column {
                        index,
                        schema: None,
                    })
                    .collect(),
                subjects: Vec::new(),
                effective_name: Some(name.to_owned()),
            });
        }
    }
    refuse(failures)?;
    Ok((name.expect("table admitted"), names))
}

pub(super) fn allocate_key(
    name: &str,
    prefix: &str,
    occupied: &BTreeSet<String>,
) -> Result<String, DesignerError> {
    let derived = name.to_ascii_lowercase();
    if {
        let mut chars = derived.chars();
        chars.next().is_some_and(|c| c.is_ascii_alphanumeric())
            && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))
    } && !occupied.contains(&derived)
    {
        return Ok(derived);
    }
    let mut suffix = 1_usize;
    loop {
        let key = format!("{prefix}_{suffix}");
        if !occupied.contains(&key) {
            return Ok(key);
        }
        suffix = suffix
            .checked_add(1)
            .ok_or_else(|| table_error("internal key allocation is exhausted"))?;
    }
}
