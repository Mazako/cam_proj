#[derive(Clone, Copy, Debug, Eq, PartialEq, sqlx::Type)]
#[sqlx(rename_all = "snake_case")]
pub enum EventStatus {
    Recording,
    Finalizing,
    Ready,
    Failed,
}
