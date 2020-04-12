use bgb_link::commands::typed::TypedBgbCommand::*;
use bgb_link::net::listener::BgbListener;
use rand::prelude::*;
use std::io;
use std::io::Write;
use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;

enum Event {
    Fuzz,
    Message(String),
}

fn main() -> io::Result<()> {
    let (tx_event, rx_event) = mpsc::channel();

    thread::spawn(move || {
        let mut bytes: u32 = 0;
        let mut bytes_string_len = 1;
        print!("Fuzzed bytes: 0");
        io::stdout().flush().unwrap();
        for event in rx_event {
            match event {
                Event::Fuzz => {
                    for _ in 0..bytes_string_len {
                        print!("\u{8}");
                        io::stdout().flush().unwrap();
                    }
                    bytes = bytes.saturating_add(1);
                    let bytes_string = format!("{}", bytes);
                    bytes_string_len = bytes_string.len();
                    print!("{}", bytes_string);
                    io::stdout().flush().unwrap();
                }
                Event::Message(e) => {
                    print!("\r{}\nFuzzed bytes: {}", e, bytes);
                    io::stdout().flush().unwrap();
                }
            }
        }
    });

    let listener = BgbListener::wrap(TcpListener::bind("127.0.0.1:8765")?);
    if let Err(_) = tx_event.send(Event::Message(String::from("Listening on 127.0.0.1:8765"))) {
        println!("Listening on 127.0.0.1:8765");
    }
    for conn in listener.incoming() {
        if let Ok(mut conn) = conn {
            let tx_event = tx_event.clone();
            thread::spawn(move || {
                if let Err(e) = conn.write(&Status {
                    running: true,
                    paused: true,
                    support_reconnect: false,
                }) {
                    if let Err(_) = tx_event.send(Event::Message(format!("Lost connection: {}", e)))
                    {
                        println!("Lost connection: {}", e);
                    }
                    return;
                }

                if let Err(e) = conn.write(&Sync2 { data: 2 }) {
                    if let Err(_) = tx_event.send(Event::Message(format!("Lost connection: {}", e)))
                    {
                        println!("Lost connection: {}", e);
                    }
                    return;
                }

                if let Err(e) = conn.write(&Status {
                    running: true,
                    paused: false,
                    support_reconnect: false,
                }) {
                    if let Err(_) = tx_event.send(Event::Message(format!("Lost connection: {}", e)))
                    {
                        println!("Lost connection: {}", e);
                    }
                    return;
                }

                let mut rng = thread_rng();
                loop {
                    match conn.read() {
                        Ok(command) => match command {
                            Sync1 {
                                data: _,
                                high_speed: _,
                                double_speed: _,
                                timestamp: _,
                            } => {
                                if let Err(e) = conn.write(&Sync2 { data: rng.gen() }) {
                                    if let Err(_) = tx_event
                                        .send(Event::Message(format!("Lost connection: {}", e)))
                                    {
                                        println!("Lost connection: {}", e);
                                    }
                                    return;
                                }
                                if let Err(_) = tx_event.send(Event::Fuzz) {}
                            }
                            Sync3Timestamp { timestamp } => {
                                if let Err(e) = conn.write(&Sync3Timestamp { timestamp }) {
                                    if let Err(_) = tx_event
                                        .send(Event::Message(format!("Lost connection: {}", e)))
                                    {
                                        println!("Lost connection: {}", e);
                                    }
                                    return;
                                }
                            }
                            _ => {}
                        },
                        Err(e) => {
                            if let Err(_) =
                                tx_event.send(Event::Message(format!("Lost connection: {}", e)))
                            {
                                println!("Lost connection: {}", e);
                            }
                            return;
                        }
                    }
                }
            });
        }
    }

    Ok(())
}
