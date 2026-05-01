use crate::lexer::Span;

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub span: Span,
    pub message: String,
    pub label: String,
}

impl Diagnostic {
    pub fn new(span: Span, message: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            span,
            message: message.into(),
            label: label.into(),
        }
    }
}

pub fn report(filename: &str, src: &str, diag: &Diagnostic) {
    use ariadne::{Color, Label, Report, ReportKind, Source};

    let _ = Report::build(ReportKind::Error, (filename, diag.span.clone()))
        .with_message(&diag.message)
        .with_label(
            Label::new((filename, diag.span.clone()))
                .with_message(&diag.label)
                .with_color(Color::Red),
        )
        .finish()
        .eprint((filename, Source::from(src)));
}
