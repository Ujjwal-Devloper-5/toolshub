
/// Launch the interactive TUI dashboard.
pub async fn run() -> anyhow::Result<()> {
    toolshub_tui::run().map_err(|e| anyhow::anyhow!("TUI error: {e}"))?;
    Ok(())
}
