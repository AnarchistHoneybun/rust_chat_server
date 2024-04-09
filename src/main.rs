use tokio::{io::{AsyncReadExt, AsyncWriteExt, BufReader}, net::TcpListener};
use tokio::io::AsyncBufReadExt;
use tokio::sync::broadcast;

#[tokio::main]
async fn main() {
    let listener: TcpListener = TcpListener::bind("localhost:8080").await.unwrap();

    let (tx, _rx) = broadcast::channel(10);

    loop {
        let (mut socket, addr) = listener.accept().await.unwrap();

        let tx = tx.clone();
        let mut rx = tx.subscribe();

        tokio::spawn(async move{
            let (read_half, mut write_half) = socket.split();

            let mut reader = BufReader::new(read_half);
            let mut line = String::new();

            loop {
                tokio::select! {
                    result = reader.read_line(&mut line) => {
                        if result.unwrap() == 0 {
                            break;
                        }

                        tx.send((line.clone(), addr)).unwrap();
                        line.clear();
                    },
                    result = rx.recv() => {
                        let (msg, other_addr ) = result.unwrap();

                        if addr!= other_addr {
                        write_half.write_all(msg.as_bytes()).await.unwrap();
                            }
                    }
                }

            }
        });
    }

}
