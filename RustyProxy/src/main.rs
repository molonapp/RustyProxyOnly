use std::env;
use std::io::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{timeout, Duration};

#[tokio::main]
async fn main() -> Result<(), Error> {
    let port = get_port();
    let listener = TcpListener::bind(format!("[::]:{}", port)).await?;
    println!("Rodando na porta {}", port);
    loop {
        let (client, addr) = listener.accept().await?;
        tokio::spawn(async move {
            if let Err(e) = handle_client(client).await {
                eprintln!("Erro com {}: {}", addr, e);
            }
        });
    }
}

async fn handle_client(mut client: TcpStream) -> Result<(), Error> {
    let status = get_status();
    client.write_all(format!("HTTP/1.1 101 {}\r\n\r\n", status).as_bytes()).await?;
    let mut buf = [0u8; 1];
    timeout(Duration::from_millis(300), client.read(&mut buf)).await.ok();

    client.write_all(format!("HTTP/1.1 200 {}\r\n\r\n", status).as_bytes()).await?;

    // Forçar para sempre redirecionar para SSH (ou troque por 1194)
    let server = TcpStream::connect("127.0.0.1:22").await?;

    let (mut cr, mut cw) = client.into_split();
    let (mut sr, mut sw) = server.into_split();

    tokio::select! {
        r = tokio::io::copy(&mut cr, &mut sw) => { r?; },
        r = tokio::io::copy(&mut sr, &mut cw) => { r?; },
    }

    Ok(())
}

fn get_port() -> u16 {
    env::args().skip_while(|a| a != "--port").nth(1).and_then(|p| p.parse().ok()).unwrap_or(80)
}

fn get_status() -> String {
    env::args().skip_while(|a| a != "--status").nth(1).unwrap_or("@RustyManager".to_string())
}
