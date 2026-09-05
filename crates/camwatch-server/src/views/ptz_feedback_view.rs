use askama::Template;

#[derive(Debug, Template)]
#[template(path = "fragments/ptz_feedback.html")]
pub struct PtzFeedbackView {
    pub message: String,
    pub message_class: &'static str,
}

impl PtzFeedbackView {
    pub fn new(message: impl Into<String>, message_class: &'static str) -> Self {
        Self {
            message: message.into(),
            message_class,
        }
    }
}
