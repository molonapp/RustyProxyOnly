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
            Err(e) => println!("Erro ao aceitar conexão: {}", e),
        }
    }
}

async fn handle_client(mut client_stream: TcpStream) -> Result<(), Error> {
    let status = get_status();
    let headers = format!(
        "HTTP/1.1 101 Switching Protocols\r\n\
        Upgrade: websocket\r\n\
        Connection: Upgrade\r\n\
        Sec-WebSocket-Accept: {}\r\n\r\n",
        base64::encode("dummy_key")
    );

    client_stream.write_all(headers.as_bytes()).await?;

    let mut peek_buf = vec![0; 1024];
    let _ = client_stream.peek(&mut peek_buf).await?;

    // Aguarda o início da comunicação real do cliente
    let mut addr_proxy = "0.0.0.0:22";
    let result = timeout(Duration::from_secs(2), peek_stream(&mut client_stream)).await
        .unwrap_or_else(|_| Ok(String::new()));

    if let Ok(data) = result {
        if data.contains("SSH") || data.is_empty() {
            addr_proxy = "0.0.0.0:22";
        } else {
            addr_proxy = "0.0.0.0:1194";
        }
    }

    let server_connect = TcpStream::connect(addr_proxy).await;
    if server_connect.is_err() {
        println!("erro ao iniciar conexão para o proxy");
        return Ok(());
    }

    let server_stream = server_connect?;

    let (client_read, client_write) = client_stream.into_split();
    let (server_read, server_write) = server_stream.into_split();

    let client_read = Arc::new(Mutex::new(client_read));
    let client_write = Arc::new(Mutex::new(client_write));
    let server_read = Arc::new(Mutex::new(server_read));
    let server_write = Arc::new(Mutex::new(server_write));

    let client_to_server = transfer_data(client_read, server_write.clone());
    let server_to_client = transfer_data(server_read, client_write.clone());

    // Adiciona ping no canal do cliente a cada 20s
    let heartbeat = async move {
        loop {
            tokio::time::sleep(Duration::from_secs(20)).await;
            let _ = client_write.lock().await.write_all(b"PING\r\n").await;
        }
    };

    tokio::try_join!(client_to_server, server_to_client, heartbeat)?;

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
    Ok(String::from_utf8_lossy(data).to_string())
}

fn get_port() -> u16 {
    let args: Vec<String> = env::args().collect();
    let mut port = 80;
    for i in 1..args.len() {
        if args[i] == "--port" && i + 1 < args.len() {
            port = args[i + 1].parse().unwrap_or(80);
        }
    }
    port
}

fn get_status() -> String {
    let args: Vec<String> = env::args().collect();
    let mut status = "@RustyManager".to_string();
    for i in 1..args.len() {
        if args[i] == "--status" && i + 1 < args.len() {
            status = args[i + 1].clone();
        }
    }
    status
}
