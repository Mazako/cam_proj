use askama::Template;

#[derive(Debug, Template)]
#[template(path = "errors/forbidden.html")]
pub struct Error403View {
    pub csrf_token: String,
    pub show_logout: bool,
}

impl Error403View {
    pub fn new() -> Self {
        Self {
            csrf_token: String::new(),
            show_logout: false,
        }
    }
}

impl Default for Error403View {
    fn default() -> Self {
        Self::new()
    }
}
