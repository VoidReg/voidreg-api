use std::io;

use voidreg_api::config::Config;
use voidreg_api::db::Db;
use voidreg_api::install_crypto;
use voidreg_api::telemetry;

fn main() -> io::Result<()> {
    install_crypto();
    dotenvy::dotenv().ok();
    telemetry::init();

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(migrate())
}

async fn migrate() -> io::Result<()> {
    let seed = std::env::args().any(|arg| arg == "--seed");
    let config = Config::from_env().map_err(|err| io::Error::other(err.to_string()))?;
    let db = Db::connect(&config)
        .await
        .map_err(|err| io::Error::other(err.to_string()))?;
    db.migrate()
        .await
        .map_err(|err| io::Error::other(err.to_string()))?;
    tracing::info!("schema applied");
    if seed {
        db.seed_if_empty()
            .await
            .map_err(|err| io::Error::other(err.to_string()))?;
        tracing::info!("seed complete");
    }
    Ok(())
}
