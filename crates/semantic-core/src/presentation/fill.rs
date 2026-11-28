/// A solid colour or an explicit override suppressing a cell's inherited fill.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CellFill {
    Solid(String),
    Clear,
}

impl CellFill {
    pub(super) fn solid_color(&self) -> Option<&str> {
        match self {
            Self::Solid(color) => Some(color),
            Self::Clear => None,
        }
    }
}
