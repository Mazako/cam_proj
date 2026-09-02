use std::{path::PathBuf, time::Duration};

use derive_new::new;

#[derive(Clone, Debug, Eq, PartialEq, new)]
pub struct Clip {
    pub path: PathBuf,
    pub duration: Duration,
}
