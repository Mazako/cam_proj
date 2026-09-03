use askama::Template;

#[derive(Debug, Template)]
#[template(path = "errors/forbidden.html")]
pub struct Error403View;
