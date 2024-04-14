use crate::{Room, UserInfo};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::net::tcp::WriteHalf;
use tokio::sync::{broadcast, Mutex as TokioMutex};


pub(crate) async fn handle_create_room_command(
    write_half: &mut WriteHalf<'_>,
    line: &str,
    username: &str,
    rooms: Arc<TokioMutex<Vec<Room>>>,
) {
    let mut parts = line.trim().split_whitespace();
    parts.next(); // skip /create
    if let Some(room_name) = parts.next() {
        if room_name == "glb" || room_name == "adm" {
            write_half.write_all(b"Room name 'glb' or 'adm' is reserved\n").await.unwrap();
        } else {
            let room = Room {
                name: room_name.to_string(),
                users: vec![],
            };
            let mut rooms_guard = rooms.lock().await;
            rooms_guard.push(room);
            drop(rooms_guard);
            println!("Room {} created by {}", room_name, username);
        }
    } else {
        write_half.write_all(b"No room name provided\n").await.unwrap();
    }
}

pub(crate) async fn handle_join_room_command(
    write_half: &mut WriteHalf<'_>,
    line: &str,
    username: &str,
    addr: std::net::SocketAddr,
    rooms: Arc<TokioMutex<Vec<Room>>>,
    users: Arc<TokioMutex<Vec<UserInfo>>>,
) {
    let mut parts = line.trim().split_whitespace();
    parts.next(); // skip /join
    if let Some(room_name) = parts.next() {
        let mut rooms_guard = rooms.lock().await;
        let room = rooms_guard.iter_mut().find(|r| r.name == room_name);
        if let Some(room) = room {
            room.users.push(UserInfo {
                username: username.parse().unwrap(),
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
    } else {
        write_half.write_all(b"No room name provided\n").await.unwrap();
    }
}

pub(crate) async fn handle_leave_room_command(
    write_half: &mut WriteHalf<'_>,
    line: &str,
    username: &str,
    rooms: Arc<TokioMutex<Vec<Room>>>,
    users: Arc<TokioMutex<Vec<UserInfo>>>,
) {
    let mut parts = line.trim().split_whitespace();
    parts.next(); // skip /leave
    if let Some(room_name) = parts.next() {
        let mut rooms_guard = rooms.lock().await;
        let room = rooms_guard.iter_mut().find(|r| r.name == room_name);
        if let Some(room) = room {
            let user_in_room = room.users.iter().find(|u| u.username == username);
            if let Some(_user_in_room) = user_in_room {
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
                write_half.write_all(b"[i] You are not a member of this room\n").await.unwrap();
            }
        } else {
            println!("Room {} does not exist", room_name);
            // write to user that the room does not exist
            write_half.write_all(format!("Room {} does not exist\n", room_name).as_bytes()).await.unwrap();
        }
    } else {
        write_half.write_all(b"No room name provided\n").await.unwrap();
    }
}

pub(crate) async fn handle_m_room_command(
    write_half: &mut WriteHalf<'_>,
    line: &str,
    username: &str,
    addr: std::net::SocketAddr,
    tx: broadcast::Sender<(String, std::net::SocketAddr)>,
    rooms: Arc<TokioMutex<Vec<Room>>>,
) {
    let mut parts = line.trim().split_whitespace();
    parts.next(); // skip /m_room
    let room_name = parts.next().unwrap();
    let message = parts.collect::<Vec<&str>>().join(" ");
    let rooms_guard = rooms.lock().await;
    let room = rooms_guard.iter().find(|r| r.name == room_name);
    if let Some(room) = room {
        let user_in_room = room.users.iter().find(|u| u.username == username);
        if let Some(_user_in_room) = user_in_room {
            let msg_with_username = format!("[{}] [{}] {}\n",room_name, username, message);
            tx.send((msg_with_username.clone(), addr)).unwrap();
        } else {
            write_half.write_all(b"[i] You are not a member of this room\n").await.unwrap();
        }
    } else {
        write_half.write_all(b"Room does not exist\n").await.unwrap();
    }
}

pub(crate) async fn handle_view_users_command(
    write_half: &mut WriteHalf<'_>,
    line: &str,
    username: &str,
    rooms: Arc<TokioMutex<Vec<Room>>>,
) {
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
}
pub(crate) async fn handle_list_command(
    write_half: &mut WriteHalf<'_>,
    users: Arc<TokioMutex<Vec<UserInfo>>>,
) {
    let users_guard = users.lock().await;
    for user in users_guard.iter() {
        write_half
            .write_all(format!("[{}]\n", user.username).as_bytes())
            .await
            .unwrap();
    }
}

pub(crate) async fn handle_report_command(
    write_half: &mut WriteHalf<'_>,
    reported_user: &str,
    username: &str,
    users: Arc<TokioMutex<Vec<UserInfo>>>,
) {
    let users_guard = users.lock().await;
    let reported_user_info = users_guard.iter().find(|u| u.username == reported_user);
    if let Some(_reported_user_info) = reported_user_info {
        println!("User {} reported {}", username, reported_user);
    } else {
        write_half
            .write_all(format!("User {} does not exist\n", reported_user).as_bytes())
            .await
            .unwrap();
    }
}

pub(crate) async fn handle_pm_command(
    write_half: &mut WriteHalf<'_>,
    recipient: &str,
    message: &str,
    sender: &str,
    tx: broadcast::Sender<(String, std::net::SocketAddr)>,
    users: Arc<TokioMutex<Vec<UserInfo>>>,
) {
    let users_guard = users.lock().await;
    let recipient_info = users_guard.iter().find(|u| u.username == recipient);
    if let Some(recipient_info) = recipient_info {
        let msg = format!("[PM] [{}] {}\n", sender, message);
        tx.send((msg.clone(), recipient_info.addr)).unwrap();
    } else {
        write_half.write_all(b"User not found\n").await.unwrap();
    }
}

pub(crate) async fn handle_help_command(write_half: &mut WriteHalf<'_>) {
    let help_text = "/list - List all connected users
/pm <username> <message> - Send a private message to any connected user
/report <username> - Report a user to the server admin
/exit - Disconnect from the server
/create_room <room_name> - Create a new chat room
/join_room <room_name> - Join an existing chat room
/leave_room <room_name> - Leave a chat room
/view_rooms - View all chat rooms
/view_users <room_name> - View users in a specific chat room
/m_room <room_name> <message> - Send a message to all users in a specific room\n";

    write_half.write_all(help_text.as_bytes()).await.unwrap();
}
