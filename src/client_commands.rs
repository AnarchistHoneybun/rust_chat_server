use crate::UserInfo;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::net::tcp::WriteHalf;
use tokio::sync::{broadcast, Mutex as TokioMutex};

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