use std::env;
use std::io::Error;
use tokio::io::{copy_bidirectional, AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{timeout, Duration};

/// Configuração via linha de comando
struct Config {
    listen_port: u16,
    ssh_port: u16,
    openvpn_port: u16,
    status: String,
}

impl Config {
    fn from_args() -> Self {
        let args: Vec<String> = env::args().collect();
        let mut cfg = Config {
            listen_port: 80,
            ssh_port: 22,
            openvpn_port: 1194,
            status: "@RustyManager".into(),
        };
        for i in 1..args.len() {
            match args[i].as_str() {
                "--port" if i + 1 < args.len() => {
                    cfg.listen_port = args[i + 1].parse().unwrap_or(80);
                }
                "--ssh-port" if i + 1 < args.len() => {
                    cfg.ssh_port = args[i + 1].parse().unwrap_or(22);
                }
                "--openvpn-port" if i + 1 < args.len() => {
                    cfg.openvpn_port = args[i + 1].parse().unwrap_or(1194);
                }
                "--status" if i + 1 < args.len() => {
                    cfg.status = args[i + 1].clone();
                }
                _ => {}
            }
        }
        cfg
    }
}

/// Protocolos suportados
enum Protocolo { SSH, OpenVPN, Desconhecido }

#[tokio::main]
async fn main() -> Result<(), Error> {
    let cfg = Config::from_args();
    let listener = TcpListener::bind(format!("[::]:{}", cfg.listen_port)).await?;
    println!("RustyProxy ouvindo na porta {}", cfg.listen_port);
    loop {
        let (stream, addr) = listener.accept().await?;
        println!("Nova conexão de {}", addr);
        let cfg = cfg.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_client(stream, cfg).await {
                eprintln!("Erro ao processar {}: {}", addr, e);
            }
        });
    }
}

async fn handle_client(mut client: TcpStream, cfg: Config) -> Result<(), Error> {
    // 1) Handshake HTTP simulado
    client
        .write_all(format!("HTTP/1.1 101 {}\r\n\r\n", cfg.status).as_bytes())
        .await?;
    client
        .write_all(format!("HTTP/1.1 200 {}\r\n\r\n", cfg.status).as_bytes())
        .await?;

    // 2) Detecta protocolo em até 1s
    let proto = match timeout(Duration::from_secs(1), peek(&mut client)).await {
        Ok(Ok(txt)) if txt.starts_with("SSH-") || txt.is_empty() => Protocolo::SSH,
        Ok(Ok(_)) => Protocolo::OpenVPN,
        _ => Protocolo::Desconhecido,
    };

    let target = match proto {
        Protocolo::SSH | Protocolo::Desconhecido => format!("127.0.0.1:{}", cfg.ssh_port),
        Protocolo::OpenVPN => format!("127.0.0.1:{}", cfg.openvpn_port),
    };
    println!("Encaminhando tráfego para {}", target);

    // 3) Conecta ao destino e faz túnel bidirecional IMEDIATAMENTE
    let mut server = TcpStream::connect(&target).await?;
    copy_bidirectional(&mut client, &mut server).await?;

    Ok(())
}

async fn peek(stream: &mut TcpStream) -> Result<String, Error> {
    let mut buf = [0u8; 512];
    let n = stream.peek(&mut buf).await?;
    Ok(String::from_utf8_lossy(&buf[..n]).to_string())
}

fn get_port() -> u16 {
    env::args()
        .skip_while(|a| a != "--port")
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(80)
}

fn get_status() -> String {
    env::args()
        .skip_while(|a| a != "--status")
        .nth(1)
        .unwrap_or_else(|| "@RustyManager".into())
}

// Permitir clonar Config
impl Clone for Config {
    fn clone(&self) -> Self {
        Config {
            listen_port: self.listen_port,
            ssh_port: self.ssh_port,
            openvpn_port: self.openvpn_port,
            status: self.status.clone(),
        }
    }
}
