use std::io;

use voidreg_api::{install_crypto, run, telemetry, worker_count};

fn main() -> io::Result<()> {
    install_crypto();
    dotenvy::dotenv().ok();
    telemetry::init();

    let workers = worker_count();
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(workers)
        .enable_all()
        .build()?
        .block_on(run(workers))
}
