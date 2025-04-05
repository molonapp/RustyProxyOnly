use std::env;
use std::io::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
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
                        eprintln!("Erro com cliente {}: {}", addr, e);
                    }
                });
            }
            Err(e) => eprintln!("Erro ao aceitar conexão: {}", e),
        }
    }
}

async fn handle_client(mut client_stream: TcpStream) -> Result<(), Error> {
    let status = get_status();
    client_stream
        .write_all(format!("HTTP/1.1 101 {}\r\n\r\n", status).as_bytes())
        .await?;

    let mut buffer = vec![0; 1024];
    let _ = client_stream.read(&mut buffer).await;
    client_stream
        .write_all(format!("HTTP/1.1 200 {}\r\n\r\n", status).as_bytes())
        .await?;

    let target = detect_target(&client_stream).await;
    let server_stream = TcpStream::connect(target).await?;

    let (mut cr, mut cw) = client_stream.into_split();
    let (mut sr, mut sw) = server_stream.into_split();

    tokio::select! {
        res = tokio::io::copy(&mut cr, &mut sw) => {
            if let Err(e) = res {
                eprintln!("Erro client -> server: {}", e);
            }
        }
        res = tokio::io::copy(&mut sr, &mut cw) => {
            if let Err(e) = res {
                eprintln!("Erro server -> client: {}", e);
            }
        }
    }

    Ok(())
}

async fn detect_target(stream: &TcpStream) -> &str {
    let mut buffer = vec![0u8; 512];
    let result = timeout(Duration::from_millis(300), stream.peek(&mut buffer)).await;

    if let Ok(Ok(n)) = result {
        let data = &buffer[..n];
        let s = String::from_utf8_lossy(data).to_lowercase();
        if s.contains("ssh") || s.is_empty() {
            "0.0.0.0:22"
        } else {
            "0.0.0.0:1194"
        }
    } else {
        "0.0.0.0:22"
    }
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

fn get_status() -> String {
    let args: Vec<String> = env::args().collect();
    for i in 0..args.len() {
        if args[i] == "--status" && i + 1 < args.len() {
            return args[i + 1].clone();
        }
    }
    "@RustyManager".to_string()
}
