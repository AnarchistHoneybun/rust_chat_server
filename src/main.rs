use local_ip_address::local_ip;
use std::sync::Arc;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpListener,
    sync::{broadcast, Mutex as TokioMutex},
};
mod client_commands;
use crate::client_commands::{handle_help_command, handle_list_command, handle_pm_command, handle_report_command};

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
                                let mut parts = line.trim().split_whitespace();
                                parts.next(); // skip /create
                                let room_name = parts.next().unwrap();
                                let room = Room {
                                    name: room_name.to_string(),
                                    users: vec![],
                                };
                                let mut rooms_guard = rooms.lock().await;
                                rooms_guard.push(room);
                                drop(rooms_guard);
                                println!("Room {} created by {}", room_name, username);
                            },
                            "/join_room" => {
                                let mut parts = line.trim().split_whitespace();
                                parts.next(); // skip /join
                                let room_name = parts.next().unwrap();
                                let mut rooms_guard = rooms.lock().await;
                                let room = rooms_guard.iter_mut().find(|r| r.name == room_name);
                                if let Some(room) = room {
                                    room.users.push(UserInfo {
                                        username: username.clone(),
                                        addr,
                                        rooms: vec![room_name.to_string()],
                                    });
                                    // add room to user's list of rooms
                                    let mut users_guard = users.lock().await;
                                    let user = users_guard.iter_mut().find(|u| u.username == username);
                                    if let Some(user) = user {
                                        user.rooms.push(room_name.to_string());
                                    }
                                    drop(users_guard);
                                    println!("User {} joined room {}", username, room_name);
                                    // write to user that they joined the room
                                    write_half.write_all(format!("You joined room {}\n", room_name).as_bytes()).await.unwrap();
                                } else {
                                    println!("Room {} does not exist", room_name);
                                    // write to user that the room does not exist
                                    write_half.write_all(format!("Room {} does not exist\n", room_name).as_bytes()).await.unwrap();
                                }
                            },
                            "/leave_room" => {
                                let mut parts = line.trim().split_whitespace();
                                parts.next(); // skip /leave
                                let room_name = parts.next().unwrap();
                                let mut rooms_guard = rooms.lock().await;
                                let room = rooms_guard.iter_mut().find(|r| r.name == room_name);
                                if let Some(room) = room {
                                    room.users.retain(|u| u.username != username);
                                    // remove room from user's list of rooms
                                    let mut users_guard = users.lock().await;
                                    let user = users_guard.iter_mut().find(|u| u.username == username);
                                    if let Some(user) = user {
                                        user.rooms.retain(|r| r != room_name);
                                    }
                                    drop(users_guard);
                                    println!("User {} left room {}", username, room_name);
                                    // write to user that they left the room
                                    write_half.write_all(format!("You left room {}\n", room_name).as_bytes()).await.unwrap();
                                } else {
                                    println!("Room {} does not exist", room_name);
                                    // write to user that the room does not exist
                                    write_half.write_all(format!("Room {} does not exist\n", room_name).as_bytes()).await.unwrap();
                                }
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

                            // TODO: add command to send messages to rooms

                            // command to send messages to rooms
                            // when used, checks if the sender is part of said room
                            // if true, sends the message to all users in the room
                            // if false, informs the user that they are not part of the room
                            "/m_room" => {
                                let mut parts = line.trim().split_whitespace();
                                parts.next(); // skip /m_room
                                let room_name = parts.next().unwrap();
                                let message = parts.collect::<Vec<&str>>().join(" ");
                                let rooms_guard = rooms.lock().await;
                                let room = rooms_guard.iter().find(|r| r.name == room_name);
                                if let Some(room) = room {
                                    let user_in_room = room.users.iter().find(|u| u.username == username);
                                    if let Some(_user_in_room) = user_in_room {
                                        for user in room.users.iter() {
                                            let msg_with_username = format!("[{}] {}\n", username, message);
                                            tx.send((msg_with_username.clone(), user.addr)).unwrap();
                                        }
                                    } else {
                                        write_half.write_all(b"[i] You are not a member of this room\n").await.unwrap();
                                    }
                                } else {
                                    write_half.write_all(b"Room does not exist\n").await.unwrap();
                                }
                            },

                            // command to view users in a room
                            // checks if the requesting user is a member of the room before listing users
                            // if not a member, the user is informed that they are not a member of the room
                            "/view_users" => {
                                let mut parts = line.trim().split_whitespace();
                                parts.next(); // skip /view_users
                                let room_name = parts.next().unwrap();
                                let rooms_guard = rooms.lock().await;
                                let room = rooms_guard.iter().find(|r| r.name == room_name);
                                if let Some(room) = room {
                                    let user_in_room = room.users.iter().find(|u| u.username == username);
                                    if let Some(_user_in_room) = user_in_room {
                                        for user in room.users.iter() {
                                            write_half
                                                .write_all(format!("[{}]\n", user.username).as_bytes())
                                                .await
                                                .unwrap();
                                        }
                                    } else {
                                        write_half.write_all(b"[i] Member lists are private. Join room to view.\n").await.unwrap();
                                    }
                                } else {
                                    write_half.write_all(b"Room does not exist\n").await.unwrap();
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

                        // TODO: check if incoming messages are from a room and format accordingly


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
