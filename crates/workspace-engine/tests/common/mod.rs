#![allow(dead_code)]

use std::collections::VecDeque;

use tachiko_semantic_core::Document;
use tachiko_workspace_engine::{IdGenerator, SemanticIdKind, StarterTemplate, create_document};

pub struct FixtureGenerator {
    document: VecDeque<String>,
    schemas: VecDeque<String>,
    fields: VecDeque<String>,
    entities: VecDeque<String>,
}

impl FixtureGenerator {
    pub fn game(document_id: &str) -> Self {
        Self {
            document: VecDeque::from([document_id.to_owned()]),
            schemas: ["characters", "economy", "items", "weapons"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            fields: [
                "level",
                "name",
                "weapon",
                "currency",
                "gold_per_match",
                "matches_for_sword",
                "upgrade_cost",
                "category",
                "grants_weapon",
                "name",
                "price",
                "attack_interval",
                "damage",
                "dps",
                "name",
                "price",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            entities: ["alric", "iron_sword", "shop", "tempered_blade"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
        }
    }

    pub fn empty(document_id: &str) -> Self {
        Self {
            document: VecDeque::from([document_id.to_owned()]),
            schemas: VecDeque::new(),
            fields: VecDeque::new(),
            entities: VecDeque::new(),
        }
    }
}

impl IdGenerator for FixtureGenerator {
    fn generate(&mut self, kind: SemanticIdKind) -> String {
        match kind {
            SemanticIdKind::Document => self.document.pop_front(),
            SemanticIdKind::Schema => self.schemas.pop_front(),
            SemanticIdKind::Field => self.fields.pop_front(),
            SemanticIdKind::Entity => self.entities.pop_front(),
        }
        .expect("fixture generator must cover every requested identity")
    }
}

pub fn game_balance_document(document_id: &str, title: &str) -> Document {
    let mut generator = FixtureGenerator::game(document_id);
    create_document(StarterTemplate::GameBalance, title, &mut generator).unwrap()
}

pub fn empty_document(document_id: &str, title: &str) -> Document {
    let mut generator = FixtureGenerator::empty(document_id);
    create_document(StarterTemplate::Empty, title, &mut generator).unwrap()
}

pub struct OneIdGenerator(pub Option<String>);

impl OneIdGenerator {
    pub fn new(id: &str) -> Self {
        Self(Some(id.to_owned()))
    }
}

impl IdGenerator for OneIdGenerator {
    fn generate(&mut self, _kind: SemanticIdKind) -> String {
        self.0.take().expect("one generated identity expected")
    }
}
