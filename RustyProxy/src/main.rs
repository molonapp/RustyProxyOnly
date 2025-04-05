use std::env;
use std::io::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use futures_util::{StreamExt, SinkExt};
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::protocol::Message;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let port = get_port();
    let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    println!("WebSocket proxy ouvindo na porta {}", port);
    start_ws(listener).await;
    Ok(())
}

async fn start_ws(listener: TcpListener) {
    loop {
        match listener.accept().await {
            Ok((stream, addr)) => {
                println!("Nova conexão de {}", addr);
                tokio::spawn(async move {
                    if let Err(e) = handle_client(stream).await {
                        println!("Erro com cliente {}: {}", addr, e);
                    }
                });
            }
            Err(e) => {
                println!("Erro ao aceitar conexão: {}", e);
            }
        }
    }
}

async fn handle_client(stream: TcpStream) -> Result<(), Error> {
    // Realiza handshake WebSocket
    let ws_stream = accept_async(stream).await?;
    println!("Handshake WebSocket bem-sucedido.");

    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    // Conecta ao servidor SSH local
    let ssh_stream = TcpStream::connect("127.0.0.1:22").await?;
    let (mut ssh_reader, mut ssh_writer) = ssh_stream.into_split();

    // WS -> SSH
    let to_ssh = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_receiver.next().await {
            if let Message::Binary(data) = msg {
                if ssh_writer.write_all(&data).await.is_err() {
                    break;
                }
            } else if let Message::Close(_) = msg {
                break;
            }
        }
    });

    // SSH -> WS
    let to_client = tokio::spawn(async move {
        let mut buf = [0u8; 1024];
        loop {
            let n = match ssh_reader.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => n,
                Err(_) => break,
            };
            if ws_sender.send(Message::Binary(buf[..n].to_vec())).await.is_err() {
                break;
            }
        }
    });

    let _ = tokio::try_join!(to_ssh, to_client);
    Ok(())
}

fn get_port() -> u16 {
    let args: Vec<String> = env::args().collect();
    for i in 1..args.len() {
        if args[i] == "--port" && i + 1 < args.len() {
            return args[i + 1].parse().unwrap_or(8080);
        }
    }
    8080
}
