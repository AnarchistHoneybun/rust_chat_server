use tokio::{io::{AsyncReadExt, AsyncWriteExt, BufReader}, net::TcpListener};
use tokio::io::AsyncBufReadExt;

#[tokio::main]
async fn main() {
    let listener: TcpListener = TcpListener::bind("localhost:8080").await.unwrap();

    loop {
        let (mut socket, _addr) = listener.accept().await.unwrap();

        tokio::spawn(async move{
            let (read_half, mut write_half) = socket.split();

            let mut reader = BufReader::new(read_half);
            let mut line = String::new();

            loop {
                let bytes_read = reader.read_line(&mut line).await.unwrap();

                if bytes_read == 0 {
                    break;
                }

                write_half.write_all(line.as_bytes()).await.unwrap();
                line.clear();
            }
        });
    }

}
