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
            Err(e) => {
                println!("Erro ao aceitar conexão: {}", e);
            }
        }
    }
}

async fn handle_client(mut client_stream: TcpStream) -> Result<(), Error> {
    let status = get_status();
    client_stream
        .write_all(format!("HTTP/1.1 101 {}\r\n\r\n", status).as_bytes())
        .await?;

    let mut buffer = vec![0; 1024];
    client_stream.read(&mut buffer).await?;
    client_stream
        .write_all(format!("HTTP/1.1 200 {}\r\n\r\n", status).as_bytes())
        .await?;

    let mut addr_proxy = "0.0.0.0:22";
    let result = timeout(Duration::from_secs(1), peek_stream(&mut client_stream)).await
        .unwrap_or_else(|_| Ok(String::new()));

    if let Ok(data) = result {
        if data.contains("SSH") || data.is_empty() {
            addr_proxy = "0.0.0.0:22";
        } else {
            addr_proxy = "0.0.0.0:1194";
        }
    } else {
        addr_proxy = "0.0.0.0:22";
    }

    let server_connect = TcpStream::connect(addr_proxy).await;
    if server_connect.is_err() {
        println!("erro ao iniciar conexão para o proxy ");
        return Ok(());
    }

    let server_stream = server_connect?;

    let (client_read, client_write) = client_stream.into_split();
    let (server_read, server_write) = server_stream.into_split();

    let client_read = Arc::new(Mutex::new(client_read));
    let client_write = Arc::new(Mutex::new(client_write));
    let server_read = Arc::new(Mutex::new(server_read));
    let server_write = Arc::new(Mutex::new(server_write));

    let client_to_server = transfer_data(client_read.clone(), server_write.clone());
    let server_to_client = transfer_data(server_read.clone(), client_write.clone());

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
            let mut read = read_stream.lock().await;
            match read.read(&mut buffer).await {
                Ok(0) => return Ok(()), // EOF
                Ok(n) => n,
                Err(e) => {
                    eprintln!("Erro na leitura: {}", e);
                    return Err(e);
                }
            }
        };

        {
            let mut write = write_stream.lock().await;
            if let Err(e) = write.write_all(&buffer[..bytes_read]).await {
                eprintln!("Erro na escrita: {}", e);
                return Err(e);
            }
        }

        // Camuflagem: envia "ruído" leve se passar tempo sem envio
        let write_clone = write_stream.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(10)).await;
            let mut fake = write_clone.lock().await;
            let _ = fake.write_all(b" \n").await;
        });
    }
}

async fn peek_stream(stream: &TcpStream) -> Result<String, Error> {
    let mut buffer = vec![0; 512];
    let result = timeout(Duration::from_millis(300), stream.read(&mut buffer)).await;
    match result {
        Ok(Ok(n)) if n > 0 => Ok(String::from_utf8_lossy(&buffer[..n]).to_string()),
        _ => Ok(String::new()),
    }
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
