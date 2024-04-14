use local_ip_address::local_ip;
use std::sync::Arc;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpListener,
    sync::{broadcast, Mutex as TokioMutex},
};
mod client_commands;
use crate::client_commands::{
    handle_create_room_command, handle_help_command, handle_join_room_command,
    handle_leave_room_command, handle_list_command, handle_m_room_command, handle_pm_command,
    handle_report_command, handle_view_users_command,
};

// Define a struct to store user information including their username
#[derive(Debug)]
struct UserInfo {
    username: String,
    addr: std::net::SocketAddr,
    rooms: Vec<String>,
}

#[derive(Debug)]
struct Room {
    name: String,
    users: Vec<UserInfo>,
}

#[tokio::main]
async fn main() {
    let local_ip = local_ip().unwrap();

    let listener_result = TcpListener::bind("localhost:8080").await;

    let listener = match listener_result {
        Ok(listener) => {
            println!("Server initialized on: {}:8080", local_ip);
            listener
        }
        Err(e) => {
            println!("Failed to bind to port 8080: {}", e);
            return;
        }
    };

    let (tx, _rx) = broadcast::channel(10);

    // let users = Arc::new(Mutex::new(vec![]));
    let users = Arc::new(TokioMutex::new(vec![]));

    let rooms = Arc::new(TokioMutex::new(vec![]));

    loop {
        let (mut socket, addr) = listener.accept().await.unwrap();
        println!("New connection from: {}", addr);

        let tx = tx.clone();
        let users = users.clone();
        let rooms = rooms.clone();
        let mut rx = tx.subscribe();

        tokio::spawn(async move {
            // Ask for username
            let username = ask_for_username(&mut socket).await.unwrap();
            println!("User {} connected from: {}", username, addr);

            // Store user information
            let user_info = UserInfo {
                username: username.clone(),
                addr,
                rooms: vec![],
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
                        if let Ok(0) = result {
                            handle_user_disconnection(&username, &addr, tx.clone(), users.clone()).await;

                            break;
                        }

                        // reconfigure if statements to a match statement

                        let words: Vec<&str> = line.trim().split_whitespace().collect();
                        let command = words.get(0).unwrap_or(&"");

                        match *command {
                            "/help" => {
                                handle_help_command(&mut write_half).await;
                            },
                            "/create_room" => {
                                handle_create_room_command(&mut write_half, &line, &username, rooms.clone()).await;
                            },
                            "/join_room" => {
                                handle_join_room_command(&mut write_half, &line, &username, addr, rooms.clone(), users.clone()).await;
                            },
                            "/leave_room" => {
                                handle_leave_room_command(&mut write_half, &line, &username, rooms.clone(), users.clone()).await;
                            },
                            "/m_room" => {
                                handle_m_room_command(&mut write_half, &line, &username, addr, tx.clone(), rooms.clone()).await;
                            },
                            "/view_users" => {
                                handle_view_users_command(&mut write_half, &line, &username, rooms.clone()).await;
                            },
                            "/view_rooms" => {
                                let rooms_guard = rooms.lock().await;
                                for room in rooms_guard.iter() {
                                    write_half
                                        .write_all(format!("[{}]\n", room.name).as_bytes())
                                        .await
                                        .unwrap();
                                }
                            },

                            "/list" => {
                                handle_list_command(&mut write_half, users.clone()).await;
                            },
                            "/report" => {
                                if let Some(reported_user) = words.get(1) {
                                    handle_report_command(&mut write_half, reported_user, &username, users.clone()).await;
                                } else {
                                    println!("User {} attempted to report, but no username was provided", username);
                                }
                            },
                            "/pm" => {
                                let mut parts = line.trim().split_whitespace();
                                parts.next(); // skip /pm
                                let recipient = parts.next().unwrap();
                                let message = parts.collect::<Vec<&str>>().join(" ");
                                handle_pm_command(&mut write_half, recipient, &message, &username, tx.clone(), users.clone()).await;
                            },
                            "/exit" => {
                                handle_user_disconnection(&username, &addr, tx.clone(), users.clone()).await;
                                break;
                            },
                            _ => {
                                println!("Broadcasting message from {}: {}", username, line);
                                let msg_with_username = format!("[glb] [{}] {}", username, line);
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


                        let msg_type = if msg.starts_with("[PM]") {
                            "PM"
                        } else if msg.starts_with("[glb]") {
                            "GLB"
                        } else if msg.starts_with("[i]") {
                            "INFO"
                        } else {
                            "ROOM"
                        };


                        match msg_type {
                            "PM" => {
                                if addr == other_addr {
                                    write_half.write_all(msg.as_bytes()).await.unwrap();
                                    println!("PM received by: {}", username);
                                }
                            },
                            "GLB" => {
                                if addr != other_addr {
                                    write_half.write_all(msg.as_bytes()).await.unwrap();
                                    println!("Global message received by: {}", username);
                                }
                            },
                            "INFO" => {
                                if addr != other_addr {
                                    write_half.write_all(msg.as_bytes()).await.unwrap();
                                }
                            },
                            "ROOM" => {
                                if addr != other_addr {
                                    // Extract room name from the message
                                let room_name = msg.split_whitespace().next().unwrap().trim_start_matches('[').trim_end_matches(']');
                                // Check if the user is in the room
                                let users_guard = users.lock().await;
                                let user = users_guard.iter().find(|u| u.username == username);
                                if let Some(user) = user {
                                    if user.rooms.contains(&room_name.to_string()) {
                                        write_half.write_all(msg.as_bytes()).await.unwrap();
                                        println!("Room message received by: {}", username);
                                    }
                                }
                                }
                            },
                            _ => {
                                println!("Unknown message type");
                            }
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

async fn handle_user_disconnection(
    username: &str,
    addr: &std::net::SocketAddr,
    tx: broadcast::Sender<(String, std::net::SocketAddr)>,
    users: Arc<TokioMutex<Vec<UserInfo>>>,
) {
    println!("{} disconnected", username);
    let dc_message = format!("[i] {} disconnected\n", username);
    tx.send((dc_message.clone(), *addr)).unwrap();

    // remove disconnected user from the list
    let mut users_guard = users.lock().await;
    users_guard.retain(|u| u.username != username);
    drop(users_guard);
}
