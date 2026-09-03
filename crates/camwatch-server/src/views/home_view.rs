use askama::Template;

#[derive(Template)]
#[template(path = "pages/home.html")]
pub struct HomeView {
    pub csrf_token: String,
}

impl HomeView {
    pub fn new(csrf_token: String) -> Self {
        Self { csrf_token }
    }
}
