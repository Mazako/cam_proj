use askama::Template;

#[derive(Debug, Template)]
#[template(path = "errors/internal.html")]
pub struct Error500View {
    pub csrf_token: String,
    pub show_logout: bool,
}

impl Error500View {
    pub fn new() -> Self {
        Self {
            csrf_token: String::new(),
            show_logout: false,
        }
    }
}

impl Default for Error500View {
    fn default() -> Self {
        Self::new()
    }
}
