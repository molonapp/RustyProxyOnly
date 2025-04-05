use std::env;
use std::io::Error;
use tokio::io::{copy_bidirectional, AsyncReadExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{timeout, Duration};
use sha1::{Sha1, Digest};
use base64::{engine::general_purpose, Engine as _};

#[tokio::main]
async fn main() -> Result<(), Error> {
    let port = env::args().skip_while(|a| a != "--port").nth(1)
        .and_then(|p| p.parse().ok()).unwrap_or(80);
    let listener = TcpListener::bind(("0.0.0.0", port)).await?;
    println!("RustyProxy ouvindo na porta {}", port);

    loop {
        let (mut client, addr) = listener.accept().await?;
        println!("Conexão de {}", addr);
        tokio::spawn(async move {
            if let Err(e) = handle_client(&mut client).await {
                eprintln!("Erro com {}: {}", addr, e);
            }
        });
    }
}

async fn handle_client(client: &mut TcpStream) -> Result<(), Error> {
    // 1) Leia o pedido do cliente (handshake WebSocket)
    let mut buf = [0u8; 2048];
    let n = client.read(&mut buf).await?;
    if n == 0 { return Ok(()); }
    let req = String::from_utf8_lossy(&buf[..n]);

    // 2) Extraia a chave
    let key = req.lines()
        .find_map(|line| {
            let l = line.to_lowercase();
            if l.starts_with("sec-websocket-key:") {
                Some(line.split(':').nth(1).unwrap().trim().to_string())
            } else { None }
        })
        .unwrap_or_default();

    // 3) Calcule o Sec-WebSocket-Accept
    let mut hasher = Sha1::new();
    hasher.update(key.as_bytes());
    hasher.update(b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11");
    let accept = general_purpose::STANDARD.encode(hasher.finalize());

    // 4) Envie o handshake real
    let response = format!(
        "HTTP/1.1 101 Switching Protocols\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Accept: {}\r\n\r\n",
        accept
    );
    client.write_all(response.as_bytes()).await?;

    // 5) Detecte protocolo (SSH vs OpenVPN) opcionalmente
    let target = {
        let mut peek_buf = [0u8; 512];
        let protocol = timeout(Duration::from_secs(1), client.peek(&mut peek_buf))
            .await.ok()
            .and_then(|r| r.ok())
            .filter(|&n| n>0)
            .map(|n| String::from_utf8_lossy(&peek_buf[..n]).to_lowercase())
            .unwrap_or_default();
        if protocol.contains("ssh") { "127.0.0.1:22" } else { "127.0.0.1:1194" }
    };

    println!("Encaminhando tráfego para {}", target);

    // 6) Conecte e faça túnel bidirecional imediatamente
    let mut server = TcpStream::connect(target).await?;
    copy_bidirectional(client, &mut server).await?;

    Ok(())
}
