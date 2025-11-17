use color_eyre::eyre;

use crate::app::App;

pub mod api;
pub mod app;
pub mod query;
pub mod results;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    let terminal = ratatui::init();

    let app = App::default();
    app.run(terminal).await?;

    ratatui::restore();

    Ok(())
}
