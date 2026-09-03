use askama::Template;

#[derive(Debug, Template)]
#[template(path = "errors/not_found.html")]
pub struct Error404View;
