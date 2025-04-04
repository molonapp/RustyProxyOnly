use std::env;
use std::io::Error;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio::time::{timeout, Duration};

#[tokio::main]
async fn main() -> Result<(), Error> {
    let port = get_port();
    let listener = TcpListener::bind(format!("[::]:{}", port)).await?;
    println!("Iniciando serviço na porta: {}", port);
    start_proxy(listener).await;
    Ok(())
}

async fn start_proxy(listener: TcpListener) {
    loop {
        match listener.accept().await {
            Ok((client_stream, addr)) => {
                tokio::spawn(async move {
                    if let Err(e) = handle_client(client_stream).await {
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

async fn handle_client(mut client_stream: TcpStream) -> Result<(), Error> {
    // ** Responde um WebSocket 101 Switching Protocols **
    let handshake = "HTTP/1.1 101 Switching Protocols\r\n\
                     Upgrade: websocket\r\n\
                     Connection: Upgrade\r\n\
                     Sec-WebSocket-Accept: dGhpcyBpcyBhIGZha2UgaGFuZHNoYWtl\r\n\
                     \r\n";
    
    let _ = client_stream.write_all(handshake.as_bytes()).await;

    // Ignora payloads malformadas, apenas faz proxy
    let addr_proxy = match timeout(Duration::from_secs(1), peek_stream(&mut client_stream)).await {
        Ok(Ok(data)) if data.contains("SSH") || data.is_empty() => "0.0.0.0:22",
        _ => "0.0.0.0:1194",
    };

    let server_stream = match TcpStream::connect(addr_proxy).await {
        Ok(s) => s,
        Err(_) => return Ok(()),
    };

    let (client_read, client_write) = client_stream.into_split();
    let (server_read, server_write) = server_stream.into_split();

    let client_read = Arc::new(Mutex::new(client_read));
    let client_write = Arc::new(Mutex::new(client_write));
    let server_read = Arc::new(Mutex::new(server_read));
    let server_write = Arc::new(Mutex::new(server_write));

    let t1 = transfer_data(client_read, server_write);
    let t2 = transfer_data(server_read, client_write);

    tokio::try_join!(t1, t2)?;
    Ok(())
}

async fn transfer_data(
    read: Arc<Mutex<tokio::net::tcp::OwnedReadHalf>>,
    write: Arc<Mutex<tokio::net::tcp::OwnedWriteHalf>>,
) -> Result<(), Error> {
    let mut buffer = [0; 8192];
    loop {
        let bytes_read = {
            let mut reader = read.lock().await;
            reader.read(&mut buffer).await?
        };

        if bytes_read == 0 {
            break;
        }

        let mut writer = write.lock().await;
        writer.write_all(&buffer[..bytes_read]).await?;
    }
    Ok(())
}

async fn peek_stream(stream: &TcpStream) -> Result<String, Error> {
    let mut peek_buf = [0u8; 1024];
    let n = stream.peek(&mut peek_buf).await?;
    Ok(String::from_utf8_lossy(&peek_buf[..n]).to_string())
}

fn get_port() -> u16 {
    let args: Vec<String> = env::args().collect();
    for i in 1..args.len() {
        if args[i] == "--port" {
            if i + 1 < args.len() {
                return args[i + 1].parse().unwrap_or(80);
            }
        }
    }
    80
}
