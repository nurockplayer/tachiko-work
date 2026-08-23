use std::{cmp::Ordering, fmt};

use serde::{Deserialize, Serialize};

use crate::{DocumentId, EntityId, FieldId, FieldRef, SchemaId};

/// Published symbolic rule identity.
///
/// The exact Rust carrier is intentionally internal and provisional. The
/// symbolic value, rather than an enum ordinal, carries machine meaning.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct DiagnosticCode(&'static str);

impl DiagnosticCode {
    #[must_use]
    pub const fn new(code: &'static str) -> Self {
        Self(code)
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl fmt::Display for DiagnosticCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

/// Opaque identity of the deterministic validator that emitted a diagnostic.
///
/// This is deliberately not a closed taxonomy: semantic-core does not know
/// about formula, workspace, domain, or future extension validators.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct DiagnosticProvider(&'static str);

impl DiagnosticProvider {
    #[must_use]
    pub const fn new(provider: &'static str) -> Self {
        Self(provider)
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl fmt::Display for DiagnosticProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

/// Machine classification, independent from any operation-specific gate.
///
/// The vocabulary remains provisional under ADR-0019.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    Error,
}

/// Stable semantic identity carried by a diagnostic.
///
/// Human keys and representation paths intentionally do not appear here.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum SemanticSubject {
    Document(DocumentId),
    Schema(SchemaId),
    SchemaField { schema: SchemaId, field: FieldId },
    Entity(EntityId),
    EntityField(FieldRef),
}

/// A code-specific, presentation-neutral machine fact.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct DiagnosticFact {
    pub name: &'static str,
    pub value: String,
}

impl DiagnosticFact {
    #[must_use]
    pub fn new(name: &'static str, value: impl Into<String>) -> Self {
        Self {
            name,
            value: value.into(),
        }
    }
}

/// Derived human-facing location. This is presentation, never identity.
#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct DiagnosticLocation(String);

impl DiagnosticLocation {
    #[must_use]
    pub fn new(path: impl Into<String>) -> Self {
        Self(path.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DiagnosticLocation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl PartialEq<&str> for DiagnosticLocation {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

/// Stable portion of a diagnostic used by conformance and deterministic order.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct StableDiagnosticObservation {
    pub code: DiagnosticCode,
    pub severity: DiagnosticSeverity,
    pub subjects: Vec<SemanticSubject>,
    pub related_subjects: Vec<SemanticSubject>,
    pub facts: Vec<DiagnosticFact>,
    pub provider: DiagnosticProvider,
}

/// A semantic-first diagnostic with optional presentation attached.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Diagnostic {
    pub code: DiagnosticCode,
    pub severity: DiagnosticSeverity,
    pub subjects: Vec<SemanticSubject>,
    pub related_subjects: Vec<SemanticSubject>,
    pub facts: Vec<DiagnosticFact>,
    pub provider: DiagnosticProvider,
    pub path: DiagnosticLocation,
    pub message: String,
}

impl Diagnostic {
    #[must_use]
    pub fn new(
        code: DiagnosticCode,
        severity: DiagnosticSeverity,
        mut subjects: Vec<SemanticSubject>,
        provider: DiagnosticProvider,
    ) -> Self {
        subjects.sort();
        subjects.dedup();
        Self {
            code,
            severity,
            subjects,
            related_subjects: Vec::new(),
            facts: Vec::new(),
            provider,
            path: DiagnosticLocation::default(),
            message: String::new(),
        }
    }

    #[must_use]
    pub fn with_related_subjects(mut self, mut subjects: Vec<SemanticSubject>) -> Self {
        subjects.sort();
        subjects.dedup();
        self.related_subjects = subjects;
        self
    }

    #[must_use]
    pub fn with_fact(mut self, fact: DiagnosticFact) -> Self {
        self.facts.push(fact);
        self.facts.sort();
        self.facts.dedup();
        self
    }

    #[must_use]
    pub fn with_presentation(
        mut self,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        self.path = DiagnosticLocation::new(path);
        self.message = message.into();
        self
    }

    #[must_use]
    pub fn stable_observation(&self) -> StableDiagnosticObservation {
        StableDiagnosticObservation {
            code: self.code,
            severity: self.severity,
            subjects: self.subjects.clone(),
            related_subjects: self.related_subjects.clone(),
            facts: self.facts.clone(),
            provider: self.provider,
        }
    }
}

impl Ord for Diagnostic {
    fn cmp(&self, other: &Self) -> Ordering {
        self.stable_observation()
            .cmp(&other.stable_observation())
            .then_with(|| self.path.cmp(&other.path))
            .then_with(|| self.message.cmp(&other.message))
    }
}

impl PartialOrd for Diagnostic {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
