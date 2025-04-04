use std::env;
use std::io::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{timeout, Duration};

#[tokio::main]
async fn main() -> Result<(), Error> {
    let args: Vec<String> = env::args().collect();
    let mut port: u16 = 80;
    let mut status: String = "@RustyManager".to_string();

    for i in 1..args.len() {
        if args[i] == "--port" {
            port = args[i + 1].parse().unwrap_or(80);
        }
        if args[i] == "--status" {
            status = args[i + 1].clone();
        }
    }

    let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    println!("Escutando na porta: {}", port);

    loop {
        let (mut client_stream, _) = listener.accept().await?;

        let status = status.clone();
        tokio::spawn(async move {
            // Envia o status de conexão
            let _ = client_stream
                .write_all(format!("HTTP/1.1 101 {}\r\n\r\n", status).as_bytes())
                .await;

            // Tenta fazer peek de forma segura (sem quebrar se malformado)
            let mut peek_buffer = vec![0u8; 1024];
            let peek_result = timeout(Duration::from_secs(2), client_stream.peek(&mut peek_buffer)).await;

            let dest = match peek_result {
                Ok(Ok(n)) if n > 0 => {
                    let payload = &peek_buffer[..n];
                    let payload_str = String::from_utf8_lossy(payload);
                    if payload_str.contains("SSH") {
                        "127.0.0.1:22"
                    } else {
                        "127.0.0.1:7300" // exemplo: BadVPN porta
                    }
                }
                _ => "127.0.0.1:7300", // fallback padrão
            };

            // Conecta ao servidor de destino
            match TcpStream::connect(dest).await {
                Ok(mut remote_stream) => {
                    let (mut cr, mut cw) = client_stream.split();
                    let (mut rr, mut rw) = remote_stream.split();

                    let client_to_remote = tokio::spawn(async move {
                        tokio::io::copy(&mut cr, &mut rw).await.ok();
                    });

                    let remote_to_client = tokio::spawn(async move {
                        tokio::io::copy(&mut rr, &mut cw).await.ok();
                    });

                    let _ = tokio::try_join!(client_to_remote, remote_to_client);
                }
                Err(e) => {
                    println!("Erro ao conectar ao destino: {}", e);
                }
            }
        });
    }
}
