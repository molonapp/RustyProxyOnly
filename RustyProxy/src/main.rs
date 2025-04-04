use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::net::SocketAddr;
use std::collections::HashMap;
use std::sync::{Arc};
use tokio::sync::Mutex;

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("0.0.0.0:7300").await.unwrap();
    println!("[+] Aguardando conexão BadVPN na porta 7300");

    loop {
        let (mut stream, addr) = listener.accept().await.unwrap();
        println!("[+] Conectado de: {}", addr);
        tokio::spawn(async move {
            if let Err(e) = handle_badvpn_connection(&mut stream).await {
                eprintln!("[x] Erro: {}", e);
            }
        });
    }
}

async fn handle_badvpn_connection(stream: &mut TcpStream) -> tokio::io::Result<()> {
    let mut conn_map: HashMap<u16, SocketAddr> = HashMap::new();
    let mut udp_socket = UdpSocket::bind("0.0.0.0:0").await?;
    let mut buf = [0u8; 1500];

    loop {
        let n = match stream.read(&mut buf).await {
            Ok(0) => return Ok(()), // EOF
            Ok(n) => n,
            Err(e) => return Err(e),
        };

        if n < 12 { continue; } // frame mínimo

        let conn_id = u16::from_be_bytes([buf[0], buf[1]]);
        let _flags = u16::from_be_bytes([buf[2], buf[3]]);
        let ip_len = u16::from_be_bytes([buf[4], buf[5]]) as usize;

        if n < 12 + ip_len { continue; }

        let ip_bytes = &buf[6..6+ip_len];
        let ip_str = ip_bytes.iter().map(|b| b.to_string()).collect::<Vec<_>>().join(".");
        let port = u16::from_be_bytes([buf[6+ip_len], buf[6+ip_len+1]]);
        let payload = &buf[6+ip_len+2..n];

        let target = format!("{}:{}", ip_str, port);
        let target_addr: SocketAddr = target.parse().unwrap();

        conn_map.insert(conn_id, target_addr);
        udp_socket.send_to(payload, target_addr).await?;

        // aguarda resposta UDP
        let mut resp_buf = [0u8; 1500];
        match tokio::time::timeout(std::time::Duration::from_millis(500), udp_socket.recv_from(&mut resp_buf)).await {
            Ok(Ok((resp_len, _))) => {
                let mut response = Vec::new();
                response.extend_from_slice(&conn_id.to_be_bytes());
                response.extend_from_slice(&[0x00, 0x00]);
                response.extend_from_slice(&(4u16.to_be_bytes()));
                response.extend_from_slice(&ip_bytes);
                response.extend_from_slice(&port.to_be_bytes());
                response.extend_from_slice(&resp_buf[..resp_len]);
                stream.write_all(&response).await?;
            },
            _ => continue,
        }
    }
}
