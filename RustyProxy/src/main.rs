use std::env;
use std::io::Error;
use std::net::UdpSocket;
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
    client_stream
        .write_all(format!("HTTP/1.1 101 {}\r\n\r\n", status).as_bytes())
        .await?;

    let mut buffer = vec![0; 1024];
    let bytes_read = client_stream.read(&mut buffer).await?;

    if bytes_read == 0 {
        return Err(Error::new(std::io::ErrorKind::UnexpectedEof, "EOF recebido"));
    }

    let request = String::from_utf8_lossy(&buffer[..bytes_read]);
    println!("Recebida requisição: {}", request);

    let udp_port = extract_udp_port(&request).unwrap_or(7100);
    println!("Encaminhando dados para porta UDP {}", udp_port);

    let udp_socket = UdpSocket::bind("0.0.0.0:0")?;
    let _ = udp_socket.send_to(&buffer[..bytes_read], format!("127.0.0.1:{}", udp_port));

    let server_addr = if request.contains("SSH") || request.is_empty() {
        "127.0.0.1:22"
    } else {
        "127.0.0.1:1194"
    };

    let server_connect = TcpStream::connect(server_addr).await?;
    let (client_read, client_write) = client_stream.into_split();
    let (server_read, server_write) = server_connect.into_split();

    let client_read = Arc::new(Mutex::new(client_read));
    let client_write = Arc::new(Mutex::new(client_write));
    let server_read = Arc::new(Mutex::new(server_read));
    let server_write = Arc::new(Mutex::new(server_write));

    let client_to_server = transfer_data(client_read, server_write);
    let server_to_client = transfer_data(server_read, client_write);

    tokio::try_join!(client_to_server, server_to_client)?;

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
                Ok(0) => break,
                Ok(n) => n,
                Err(_) => break,
            }
        };

        let mut write_guard = write_stream.lock().await;
        if let Err(_) = write_guard.write_all(&buffer[..bytes_read]).await {
            break;
        }
    }
    Ok(())
}

fn extract_udp_port(request: &str) -> Option<u16> {
    let default_ports = [7100, 7200, 7800, 7900];
    for port in default_ports {
        if request.contains(&format!(":{} ", port)) {
            return Some(port);
        }
    }
    None
}

fn get_port() -> u16 {
    env::args()
        .skip_while(|arg| arg != "--port")
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(80)
}

fn get_status() -> String {
    env::args()
        .skip_while(|arg| arg != "--status")
        .nth(1)
        .unwrap_or_else(|| "@RustyManager".to_string())
}
