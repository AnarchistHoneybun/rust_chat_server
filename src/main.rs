use tokio::{io::{AsyncBufReadExt, AsyncWriteExt, BufReader}, net::TcpListener};
use tokio::sync::{broadcast, Mutex as TokioMutex};
use std::sync::{Arc};
use local_ip_address::local_ip;


// Define a struct to store user information including their username
#[derive(Debug)]
struct UserInfo {
    username: String,
    addr: std::net::SocketAddr,
}

#[tokio::main]
async fn main() {

    let local_ip = local_ip().unwrap();

    let listener_result = TcpListener::bind("localhost:8080").await;

    let listener = match listener_result {
        Ok(listener) => {
            println!("Server initialized on: {}:8080", local_ip);
            listener
        },
        Err(e) => {
            println!("Failed to bind to port 8080: {}", e);
            return;
        }
    };

    let (tx, _rx) = broadcast::channel(10);

    // let users = Arc::new(Mutex::new(vec![]));
    let users = Arc::new(TokioMutex::new(vec![]));

    loop {
        let (mut socket, addr) = listener.accept().await.unwrap();
        println!("New connection from: {}", addr);

        let tx = tx.clone();
        let users = users.clone();
        let mut rx = tx.subscribe();

        tokio::spawn(async move{
            // Ask for username
            let username = ask_for_username(&mut socket).await.unwrap();
            println!("User {} connected from: {}", username, addr);

            // Store user information
            let user_info = UserInfo {
                username: username.clone(),
                addr,
            };

            // Add user to the list of users
            let mut users_guard = users.lock().await;
            users_guard.push(user_info);
            drop(users_guard);

            let (read_half, mut write_half) = socket.split();

            let mut reader = BufReader::new(read_half);
            let mut line = String::new();

            loop {
                tokio::select! {
                    result = reader.read_line(&mut line) => {
                        if result.unwrap() == 0 {
                            println!("{} disconnected", username);
                            let dc_message = format!("[i] {} disconnected\n", username);
                            tx.send((dc_message.clone(), addr)).unwrap();

                            // remove disconnected user from the list
                            let mut users_guard = users.lock().await;
                            users_guard.retain(|u| u.username != username);
                            drop(users_guard);

                            break;
                        }

                        // reconfigure if statements to a match statement

                        let words: Vec<&str> = line.trim().split_whitespace().collect();
                        let command = words.get(0).unwrap_or(&"");

                        match *command {
                            "/list" => {
                                let users_guard = users.lock().await;
                                for user in users_guard.iter() {
                                    write_half.write_all(format!("[{}]\n", user.username).as_bytes()).await.unwrap();
                                }
                            },
                            "/report" => {
                                if let Some(reported_user) = words.get(1) {
                                    let users_guard = users.lock().await;
                                    let reported_user_info = users_guard.iter().find(|u| u.username == *reported_user);
                                    if let Some(_reported_user_info) = reported_user_info {
                                        println!("User {} reported {}", username, reported_user);
                                    } else {
                                        write_half.write_all(format!("User {} does not exist\n", reported_user).as_bytes()).await.unwrap();
                                    }
                                } else {
                                    println!("User {} attempted to report, but no username was provided", username);
                                }
                            },
                            "/pm" => {
                                let mut parts = line.trim().split_whitespace();
                                parts.next(); // skip /pm
                                let recipient = parts.next().unwrap();
                                let message = parts.collect::<Vec<&str>>().join(" ");
                                let sender = username.clone();

                                let users_guard = users.lock().await;
                                let recipient_info = users_guard.iter().find(|u| u.username == recipient);
                                if let Some(recipient_info) = recipient_info {
                                    let msg = format!("[PM] [{}] {}\n", sender, message);
                                    tx.send((msg.clone(), recipient_info.addr)).unwrap();
                                } else {
                                    write_half.write_all(b"User not found\n").await.unwrap();
                                }
                            },
                            _ => {
                                println!("Broadcasting message from {}: {}", username, line);
                                let msg_with_username = format!("[{}] {}", username, line);
                                tx.send((msg_with_username.clone(), addr)).unwrap();
                            }
                        };

                        line.clear();
                        continue;

                    },
                    result = rx.recv() => {
                        let (msg, other_addr ) = result.unwrap();

                        // check if incoming message is a private message
                        // private messages start with [PM]

                        if msg.starts_with("[PM]") {
                            if addr == other_addr{
                                write_half.write_all(msg.as_bytes()).await.unwrap();
                                println!("Msg received by: {}", username);
                            }
                            continue;
                        }

                        if addr != other_addr{
                            write_half.write_all(msg.as_bytes()).await.unwrap();
                            println!("Msg received by: {}", username);
                        }

                    }
                }

            }
        });
    }

}

async fn ask_for_username(socket: &mut tokio::net::TcpStream) -> Result<String, std::io::Error> {
    let mut username = String::new();

    // Send a message asking for username
    socket.write_all(b"Please enter your username: ").await?;

    // Read the username from the client
    let mut reader = BufReader::new(socket);
    reader.read_line(&mut username).await?;

    Ok(username.trim().to_string())
}
