use toolshub_core::Result;

pub async fn run() -> Result<()> {
    let mut app = toolshub_tui::App::new();
    app.run()?;
    Ok(())
}
