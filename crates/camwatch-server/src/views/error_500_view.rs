use askama::Template;

#[derive(Debug, Template)]
#[template(path = "errors/internal.html")]
pub struct Error500View;
