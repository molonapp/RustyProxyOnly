use std::env;
use std::io::Error;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let port = get_port();
    let dest = get_target();
    let listener = TcpListener::bind(format!("[::]:{}", port)).await?;
    println!("Iniciando proxy burro na porta {} -> {}", port, dest);
    start_proxy(listener, dest).await;
    Ok(())
}

async fn start_proxy(listener: TcpListener, dest: String) {
    loop {
        match listener.accept().await {
            Ok((client_stream, addr)) => {
                let dest_clone = dest.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_client(client_stream, dest_clone).await {
                        println!("Erro com cliente {}: {}", addr, e);
                    }
                });
            }
            Err(e) => println!("Erro ao aceitar conexão: {}", e),
        }
    }
}

async fn handle_client(mut client_stream: TcpStream, addr_proxy: String) -> Result<(), Error> {
    let server_stream = match TcpStream::connect(addr_proxy).await {
        Ok(stream) => stream,
        Err(e) => {
            println!("Falha ao conectar ao destino: {}", e);
            return Ok(());
        }
    };

    let (client_read, client_write) = client_stream.into_split();
    let (server_read, server_write) = server_stream.into_split();

    let client_read = Arc::new(Mutex::new(client_read));
    let client_write = Arc::new(Mutex::new(client_write));
    let server_read = Arc::new(Mutex::new(server_read));
    let server_write = Arc::new(Mutex::new(server_write));

    let c2s = transfer_data(client_read, server_write);
    let s2c = transfer_data(server_read, client_write);

    tokio::try_join!(c2s, s2c)?;

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

fn get_target() -> String {
    let args: Vec<String> = env::args().collect();
    let mut target = "127.0.0.1:22".to_string(); // padrão

    for i in 1..args.len() {
        if args[i] == "--target" && i + 1 < args.len() {
            target = args[i + 1].clone();
        }
    }

    target
}
