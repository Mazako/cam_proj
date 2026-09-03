use askama::Template;

#[derive(Template)]
#[template(path = "auth/login.html")]
pub struct LoginView {
    pub csrf_token: String,
    pub show_error: bool,
}

impl LoginView {
    pub fn new(csrf_token: String, show_error: bool) -> Self {
        Self {
            csrf_token,
            show_error,
        }
    }
}
