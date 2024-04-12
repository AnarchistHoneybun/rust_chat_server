# Rust TCP Chat Server

A simple client agnostic TCP chat server written in Rust. Utilizes the `tokio` crate for concurrent and asynchronous
processing of connections and client inputs.

## Features

- [x] Multi-client connection
- [x] Asynchronous message processing
- [x] Concurrent connection/disconnection handling
- [x] Usernames and private messaging

### Planned Features

- [ ] Allows users to create rooms
- [ ] Allows users to report other users
- [ ] Admin user with special privileges

## Usage

- Requires [Rust](https://www.rust-lang.org/tools/install) and [Cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html) to be installed
- Clone the repository using `git clone https://github.com/AnarchistHoneybun/rust_chat_server.git`
- Add dependencies using `cargo add tokio local-ip-address`
- Run the server using `cargo run`

_If the server starts successfully, your local ip address and port will be displayed
in the console._ 

- Clients can connect to the server using `telnet <your-ip> <port>`
- Clients will have to enter username when prompted

_The code is tested for telnet connections, but in essence it should not matter 
what client is used to connect. In case of any errors, please open an issue._

## Client Commands

- `/list` - List all connected users
- `/pm <username> <message>` - Send a private message to any connected user
- `/report <username>` - Report a user to the server admin
