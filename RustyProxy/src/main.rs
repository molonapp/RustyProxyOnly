// Cargo.toml
// [dependencies]
// tokio = { version = "1", features = ["full"] }
// tokio-tungstenite = "0.18"
// tungstenite = "0.20"

use std::env;
use std::io::Error;
use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::protocol::Message;
use tokio_tungstenite::accept_async;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let port = env::args().nth(1).unwrap_or("80".into());
    let addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&addr).await?;
    println!("WebSocket proxy ouvindo em {}", addr);

    while let Ok((stream, peer)) = listener.accept().await {
        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream).await {
                eprintln!("Erro com {}: {}", peer, e);
            }
        });
    }
    Ok(())
}

async fn handle_connection(stream: TcpStream) -> Result<(), Error> {
    // 1) Aceita o handshake WebSocket completo
    let ws_stream = accept_async(stream).await.map_err(|e| {
        eprintln!("Handshake falhou: {}", e);
        Error::new(std::io::ErrorKind::Other, "Handshake falhou")
    })?;
    println!("WebSocket conectado");

    // 2) Conecta ao servidor SSH
    let mut ssh = TcpStream::connect("127.0.0.1:22").await?;
    println!("Conectado ao SSH");

    // 3) Separe WebSocket e SSH em duas tarefas
    let (mut ws_sink, mut ws_stream) = ws_stream.split();
    let (mut ssh_read, mut ssh_write) = ssh.split();

    // WebSocket → SSH
    let ws_to_ssh = async {
        while let Some(msg) = ws_stream.next().await {
            let msg = msg.map_err(|e| { eprintln!("WS erro: {}", e); std::io::Error::new(std::io::ErrorKind::Other, "WS") })?;
            if let Message::Binary(bin) = msg {
                ssh_write.write_all(&bin).await?;
            } else if let Message::Close(_) = msg {
                break;
            }
        }
        Ok::<_, Error>(())
    };

    // SSH → WebSocket
    let ssh_to_ws = async {
        let mut buf = [0; 8192];
        loop {
            let n = ssh_read.read(&mut buf).await?;
            if n == 0 { break; }
            ws_sink.send(Message::Binary(buf[..n].to_vec())).await.map_err(|e| {
                eprintln!("WS send erro: {}", e);
                Error::new(std::io::ErrorKind::Other, "WS send erro")
            })?;
        }
        Ok::<_, Error>(())
    };

    // 4) Rode ambas simultaneamente
    tokio::try_join!(ws_to_ssh, ssh_to_ws)?;
    println!("Conexão encerrada");
    Ok(())
}
