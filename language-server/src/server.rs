use tokio::io::{Stdin, Stdout};
use tower_lsp::{ClientSocket, LspService, Server};

use crate::backend::Backend;

pub fn create_service() -> (LspService<Backend>, ClientSocket) {
    let (service, socket) = LspService::new(|client|
        Backend::new(client)
    );
    (service, socket)
}

pub async fn start_server(service: LspService<Backend>, socket: ClientSocket) {
    let stdin: Stdin = tokio::io::stdin();
    let stdout: Stdout = tokio::io::stdout();

    let server = Server::new(stdin, stdout, socket);
    server.serve(service).await;
    ;
}
