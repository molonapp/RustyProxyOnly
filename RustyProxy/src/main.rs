use std::env;
use std::io::Error;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio::{time::{Duration}};
use tokio::time::timeout;

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Iniciando o proxy
    let port = get_port();
    let listener = TcpListener::bind(format!("[::]:{}", port)).await?;
    println!("Iniciando serviço na porta: {}", port);
    start_http(listener).await;
    Ok(())
}

async fn start_http(listener: TcpListener) {
    loop {
        match listener.accept().await {
            Ok((client_stream, addr)) => {
                tokio::spawn(async move {
                    if let Err(e) = handle_client(client_stream).await {
                        println!("Erro ao processar cliente {}: {}", addr, e);
                    }
                });
            }
            Err(e) => {
                println!("Erro ao aceitar conexão: {}", e);
            }
        }
    }
}
async fn handle_client(mut client_stream: TcpStream) -> Result<(), Error> {
    let mut addr_proxy = "0.0.0.0:22";
    let mut initial_data = Vec::new();

    // Tenta detectar WebSocket
    let result = timeout(Duration::from_secs(3), peek_stream(&mut client_stream)).await
        .unwrap_or_else(|_| Ok(String::new()));

    if let Ok(data) = result {
        if data.contains("Upgrade: websocket") || data.contains("GET") {
            // Handshake básico para WebSocket (simplificado)
            let response = "HTTP/1.1 101 Switching Protocols\r\n\
                            Upgrade: websocket\r\n\
                            Connection: Upgrade\r\n\
                            \r\n";
            client_stream.write_all(response.as_bytes()).await?;
            addr_proxy = "0.0.0.0:1194";
        }

        // Lê os dados reais agora (consome o buffer)
        let mut temp_buf = vec![0; 8192];
        let bytes_read = client_stream.read(&mut temp_buf).await?;
        if bytes_read > 0 {
            initial_data.extend_from_slice(&temp_buf[..bytes_read]);
        }
    }

    let server_stream = TcpStream::connect(addr_proxy).await?;
    let (mut client_read, mut client_write) = client_stream.into_split();
    let (mut server_read, mut server_write) = server_stream.into_split();

    // Repassa dados iniciais do cliente (úteis se ele mandou algo junto do upgrade)
    if !initial_data.is_empty() {
        server_write.write_all(&initial_data).await?;
    }

    // Comunicação bidirecional normal
    let client_to_server = tokio::spawn(async move {
        let mut buffer = [0; 8192];
        loop {
            let bytes_read = client_read.read(&mut buffer).await?;
            if bytes_read == 0 {
                break;
            }
            server_write.write_all(&buffer[..bytes_read]).await?;
        }
        Ok::<(), Error>(())
    });

    let server_to_client = tokio::spawn(async move {
        let mut buffer = [0; 8192];
        loop {
            let bytes_read = server_read.read(&mut buffer).await?;
            if bytes_read == 0 {
                break;
            }
            client_write.write_all(&buffer[..bytes_read]).await?;
        }
        Ok::<(), Error>(())
    });

    let _ = tokio::try_join!(client_to_server, server_to_client)?;
    Ok(())
}

async fn transfer_data(
    read_stream: Arc<Mutex<tokio::net::tcp::OwnedReadHalf>>,
    write_stream: Arc<Mutex<tokio::net::tcp::OwnedWriteHalf>>,
) -> Result<(), Error> {
    let mut buffer = [0; 8192];
    loop {
        let bytes_read = {
            let mut read_guard = read_stream.lock().await;
            read_guard.read(&mut buffer).await?
        };

        if bytes_read == 0 {
            break;
        }

        let mut write_guard = write_stream.lock().await;
        write_guard.write_all(&buffer[..bytes_read]).await?;
    }

    Ok(())
}

async fn peek_stream(stream: &TcpStream) -> Result<String, Error> {
    let mut peek_buffer = vec![0; 8192];
    let bytes_peeked = stream.peek(&mut peek_buffer).await?;
    let data = &peek_buffer[..bytes_peeked];
    let data_str = String::from_utf8_lossy(data);
    Ok(data_str.to_string())
}


fn get_port() -> u16 {
    let args: Vec<String> = env::args().collect();
    let mut port = 80;

    for i in 1..args.len() {
        if args[i] == "--port" {
            if i + 1 < args.len() {
                port = args[i + 1].parse().unwrap_or(80);
            }
        }
    }

    port
}

fn get_status() -> String {
    let args: Vec<String> = env::args().collect();
    let mut status = String::from("@RustyManager");

    for i in 1..args.len() {
        if args[i] == "--status" {
            if i + 1 < args.len() {
                status = args[i + 1].clone();
            }
        }
    }

    status
}
