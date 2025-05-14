mod backend;
mod server;

use log::debug;

#[tokio::main]
async fn main() {
    env_logger::init();

    let (service, socket) = server::create_service();

    debug!("Starting server: {:#?} on socket: {:#?}", service, socket);

    server::start_server(service, socket).await;
}
