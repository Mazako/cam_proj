use askama::Template;

#[derive(Debug, Template)]
#[template(path = "errors/not_found.html")]
pub struct Error404View {
    pub csrf_token: String,
    pub show_logout: bool,
}

impl Error404View {
    pub fn new() -> Self {
        Self {
            csrf_token: String::new(),
            show_logout: false,
        }
    }
}

impl Default for Error404View {
    fn default() -> Self {
        Self::new()
    }
}
