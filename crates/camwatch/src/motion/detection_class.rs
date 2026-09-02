#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectionClass {
    Person,
    Cat,
    Dog,
}

impl DetectionClass {
    pub(super) fn from_class_id(class_id: i32) -> Option<Self> {
        match class_id {
            0 => Some(Self::Person),
            15 => Some(Self::Cat),
            16 => Some(Self::Dog),
            _ => None,
        }
    }
}
