use std::env;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

const XOR_KEY: u8 = 0x5A;

fn xor(data: &mut [u8]) {
    for byte in data.iter_mut() {
        *byte ^= XOR_KEY;
    }
}

fn handle_client(mut client_stream: TcpStream, remote_host: String, remote_port: u16) {
    match TcpStream::connect(format!("{}:{}", remote_host, remote_port)) {
        Ok(mut remote_stream) => {
            let mut client_clone = client_stream.try_clone().unwrap();
            let mut remote_clone = remote_stream.try_clone().unwrap();

            let t1 = thread::spawn(move || {
                let mut buf = [0; 4096];
                loop {
                    match client_clone.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            let mut data = buf[..n].to_vec();
                            xor(&mut data);
                            if remote_stream.write_all(&data).is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
            });

            let t2 = thread::spawn(move || {
                let mut buf = [0; 4096];
                loop {
                    match remote_clone.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            let mut data = buf[..n].to_vec();
                            xor(&mut data);
                            if client_stream.write_all(&data).is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
            });

            t1.join().ok();
            t2.join().ok();
        }
        Err(e) => {
            eprintln!("Erro ao conectar ao servidor remoto: {}", e);
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 4 {
        eprintln!("Uso: {} <host_remoto> <porta_local> <porta_remota>", args[0]);
        return;
    }

    let remote_host = args[1].clone();
    let local_port: u16 = args[2].parse().expect("Porta local inválida");
    let remote_port: u16 = args[3].parse().expect("Porta remota inválida");

    let listener = TcpListener::bind(format!("0.0.0.0:{}", local_port)).expect("Erro ao iniciar listener");

    println!("Escutando na porta {} e redirecionando para {}:{}", local_port, remote_host, remote_port);

    for stream in listener.incoming() {
        match stream {
            Ok(client_stream) => {
                let remote_host = remote_host.clone();
                thread::spawn(move || {
                    handle_client(client_stream, remote_host, remote_port);
                });
            }
            Err(e) => {
                eprintln!("Erro ao aceitar conexão: {}", e);
            }
        }
    }
}
