pub fn get_reporter() -> &'static miette::GraphicalReportHandler {
    static REPORTER: std::sync::OnceLock<miette::GraphicalReportHandler> =
        std::sync::OnceLock::new();
    REPORTER.get_or_init(|| {
        miette::GraphicalReportHandler::new_themed(miette::GraphicalTheme::unicode_nocolor())
            .with_width(80)
    })
}

pub fn render(error: &dyn miette::Diagnostic) -> String {
    let mut result = String::new();
    get_reporter().render_report(&mut result, error).unwrap();
    result
}
