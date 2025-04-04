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
    println!("Servidor iniciado na porta: {}", port);
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
    client_stream.write_all(b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\r\n").await?;
    
    let mut buffer = vec![0; 1024];
    match client_stream.read(&mut buffer).await {
        Ok(0) => return Err(Error::new(std::io::ErrorKind::UnexpectedEof, "EOF recebido")),
        Ok(_) => {
            client_stream.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n").await?;
        }
        Err(e) => return Err(e),
    }

    let addr_proxy = "0.0.0.0:22";
    let server_stream = TcpStream::connect(addr_proxy).await?;

    let (client_read, client_write) = client_stream.into_split();
    let (server_read, server_write) = server_stream.into_split();

    let client_read = Arc::new(Mutex::new(client_read));
    let client_write = Arc::new(Mutex::new(client_write));
    let server_read = Arc::new(Mutex::new(server_read));
    let server_write = Arc::new(Mutex::new(server_write));

    let client_to_server = transfer_data(client_read.clone(), server_write.clone());
    let server_to_client = transfer_data(server_read.clone(), client_write.clone());
    let keep_alive = keep_connection_alive(client_write.clone());

    tokio::try_join!(client_to_server, server_to_client, keep_alive)?;

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
            match read_guard.read(&mut buffer).await {
                Ok(0) => return Err(Error::new(std::io::ErrorKind::UnexpectedEof, "EOF recebido")),
                Ok(n) => n,
                Err(_) => return Ok(()),
            }
        };
        let mut write_guard = write_stream.lock().await;
        write_guard.write_all(&buffer[..bytes_read]).await?;
    }
}

async fn keep_connection_alive(write_stream: Arc<Mutex<tokio::net::tcp::OwnedWriteHalf>>) -> Result<(), Error> {
    loop {
        tokio::time::sleep(Duration::from_secs(5)).await;
        let mut write_guard = write_stream.lock().await;
        if write_guard.write_all(b"PING\r\n").await.is_err() {
            return Err(Error::new(std::io::ErrorKind::BrokenPipe, "Conexão perdida"));
        }
    }
}

fn get_port() -> u16 {
    let args: Vec<String> = env::args().collect();
    args.iter()
        .position(|x| x == "--port")
        .and_then(|i| args.get(i + 1).and_then(|p| p.parse().ok()))
        .unwrap_or(80)
}
