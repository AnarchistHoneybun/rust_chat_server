use tokio::{io::{AsyncBufReadExt, AsyncWriteExt, BufReader}, net::TcpListener};
use tokio::sync::{broadcast, Mutex};
use std::sync::Arc;



// Define a struct to store user information including their username
#[derive(Debug)]
struct UserInfo {
    username: String,
    addr: std::net::SocketAddr,
}

#[tokio::main]
async fn main() {
    let listener: TcpListener = TcpListener::bind("localhost:8080").await.unwrap();

    let (tx, _rx) = broadcast::channel(10);

    let users = Arc::new(Mutex::new(vec![]));

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

            let (read_half, mut write_half) = socket.split();

            let mut reader = BufReader::new(read_half);
            let mut line = String::new();

            loop {
                tokio::select! {
                    result = reader.read_line(&mut line) => {
                        if result.unwrap() == 0 {
                            break;
                        }

                        // // Broadcast the message with the username
                        // println!("Broadcasting message from {}: {}", username, line);
                        // let mut receivers_count = 0;
                        // for user in users_guard.iter() {
                        //     let message = line.trim();
                        //     let msg_with_username = format!("{}: {}", username, message);
                        //     if user.addr != users_guard[0].addr {
                        //         tx.send((msg_with_username.clone(), user.addr)).unwrap();
                        //         receivers_count += 1;
                        //     }
                        //
                        // }
                        // println!("Message broadcasted to {} users", receivers_count);
                        // broadcast_message(&users, &username, &line, &tx).await;
                        tx.send((line.clone(), addr)).unwrap();

                        line.clear();
                    },
                    result = rx.recv() => {
                        let (msg, other_addr ) = result.unwrap();

                        println!("Received message from another connection: {:?}", msg);


                        if addr != other_addr{write_half.write_all(msg.as_bytes()).await.unwrap();}
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

async fn broadcast_message(users: &Arc<Mutex<Vec<UserInfo>>>, username: &String, message: &String, tx: &broadcast::Sender<(String, std::net::SocketAddr)>) {
    println!("Broadcasting message from {}: {}", username, message);
    let users_guard = users.lock().await;

    println!("Broadcasting message from {}: {}", username, message);

    // Check if the users vector is empty
    if users_guard.is_empty() {
        println!("No users connected!");
        return;
    }

    // Iterate over connected users and send the message
    let mut receivers_count = 0;
    for user in users_guard.iter() {
        let message = message.trim();
        let msg_with_username = format!("{}: {}", username, message);
        tx.send((msg_with_username.clone(), user.addr)).unwrap();
        receivers_count += 1;

    }

    println!("Message broadcasted to {} users", receivers_count);
}