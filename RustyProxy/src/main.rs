use std::env;
use std::io::Error;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

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
                        println!("Erro com {}: {}", addr, e);
                    }
                });
            }
            Err(e) => {
                println!("Erro ao aceitar conexão: {}", e);
            }
        }
    }
}

async fn handle_client(client_stream: TcpStream) -> Result<(), Error> {
    // Aqui você pode definir o destino fixo ou decidir por porta de entrada, args, etc
    let addr_proxy = get_target(); // Ex: "127.0.0.1:22" para SSH

    let server_stream = TcpStream::connect(addr_proxy).await?;
    let (client_read, client_write) = client_stream.into_split();
    let (server_read, server_write) = server_stream.into_split();

    let client_read = Arc::new(Mutex::new(client_read));
    let client_write = Arc::new(Mutex::new(client_write));
    let server_read = Arc::new(Mutex::new(server_read));
    let server_write = Arc::new(Mutex::new(server_write));

    let c_to_s = transfer_data(client_read, server_write.clone());
    let s_to_c = transfer_data(server_read, client_write.clone());

    tokio::try_join!(c_to_s, s_to_c)?;
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

fn get_target() -> &'static str {
    // Altere para o endereço de destino que deseja rotear
    "127.0.0.1:22"
}
