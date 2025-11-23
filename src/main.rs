use color_eyre::eyre;

use crate::app::App;

pub mod api;
pub mod app;
pub mod buffers;
pub mod query;
pub mod result_widget;
pub mod results;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    let terminal = ratatui::init();

    App::run(terminal).await?;

    ratatui::restore();

    Ok(())
}
