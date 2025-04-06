use std::env;
use std::io::Error;
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_tungstenite::tungstenite::protocol::Message;
use tokio_tungstenite::accept_hdr_async;
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};
use futures_util::{StreamExt, SinkExt};

#[tokio::main]
async fn main() -> Result<(), Error> {
    let port = get_port();
    let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    println!("Servidor WebSocket iniciado na porta {}", port);

    loop {
        let (stream, addr) = listener.accept().await?;
        println!("Nova conexão de: {}", addr);

        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream).await {
                println!("Erro na conexão com {}: {}", addr, e);
            }
        });
    }
}

async fn handle_connection(stream: TcpStream) -> Result<(), Error> {
    let ws_stream = accept_hdr_async(stream, |req: &Request, mut res: Response| {
        println!("Handshake recebido de {}", req.uri());
        Ok(res)
    }).await?;
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    let mut target_stream = TcpStream::connect("127.0.0.1:22").await?;
    let (mut target_reader, mut target_writer) = target_stream.split();

    // WS -> SSH
    let ws_to_ssh = async {
        while let Some(msg) = ws_receiver.next().await {
            let msg = msg?;
            if let Message::Binary(data) = msg {
                target_writer.write_all(&data).await?;
            }
        }
        Ok::<_, Error>(())
    };

    // SSH -> WS
    let ssh_to_ws = async {
        let mut buf = [0u8; 1024];
        loop {
            let n = target_reader.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            ws_sender.send(Message::Binary(buf[..n].to_vec())).await?;
        }
        Ok::<_, Error>(())
    };

    tokio::try_join!(ws_to_ssh, ssh_to_ws)?;

    Ok(())
}

fn get_port() -> u16 {
    let args: Vec<String> = env::args().collect();
    for i in 0..args.len() {
        if args[i] == "--port" && i + 1 < args.len() {
            return args[i + 1].parse().unwrap_or(80);
        }
    }
    80
}
