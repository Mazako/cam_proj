#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RemoteObject {
    pub key: String,
    pub etag: Option<String>,
    pub verbose: bool,
}
