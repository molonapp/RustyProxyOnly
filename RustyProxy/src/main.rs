use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("[::]:80").await.unwrap();
    println!("Servidor rodando na porta 80");

    loop {
        let (stream, addr) = listener.accept().await.unwrap();
        println!("Nova conexão de {}", addr);
        
        tokio::spawn(async move {
            if let Err(e) = handle_client(stream).await {
                println!("Erro na conexão com {}: {}", addr, e);
            }
        });
    }
}

async fn handle_client(mut client_stream: TcpStream) -> Result<(), std::io::Error> {
    let (mut read_half, mut write_half) = client_stream.split();

    let read_half = Arc::new(Mutex::new(read_half));
    let write_half = Arc::new(Mutex::new(write_half));

    let ping_task = keep_alive(write_half.clone());
    let read_task = listen_client(read_half, write_half);

    tokio::select! {
        _ = ping_task => {},
        _ = read_task => {},
    }

    Ok(())
}

async fn listen_client(
    read_stream: Arc<Mutex<tokio::net::tcp::OwnedReadHalf>>,
    write_stream: Arc<Mutex<tokio::net::tcp::OwnedWriteHalf>>,
) -> Result<(), std::io::Error> {
    let mut buffer = [0; 1024];

    loop {
        let bytes_read = {
            let mut read_guard = read_stream.lock().await;
            match read_guard.read(&mut buffer).await {
                Ok(0) => {
                    println!("EOF detectado, mantendo conexão...");
                    continue;
                }
                Ok(n) => n,
                Err(e) => {
                    println!("Erro ao ler do cliente: {}", e);
                    break;
                }
            }
        };

        let message = String::from_utf8_lossy(&buffer[..bytes_read]);
        println!("Recebido: {}", message);

        if message.contains("PING") {
            let mut write_guard = write_stream.lock().await;
            write_guard.write_all(b"PONG\r\n").await?;
            println!("Enviado: PONG");
        }
    }

    Ok(())
}

async fn keep_alive(write_stream: Arc<Mutex<tokio::net::tcp::OwnedWriteHalf>>) {
    loop {
        sleep(Duration::from_secs(3)).await;

        let mut write_guard = write_stream.lock().await;
        if let Err(_) = write_guard.write_all(b"PING\r\n").await {
            println!("Erro ao enviar PING. Pode ter sido desconectado.");
            break;
        }

        println!("Enviado: PING");
    }
}
