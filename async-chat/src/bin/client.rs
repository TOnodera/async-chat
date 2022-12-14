use std::sync::Arc;

use async_chat::utils::{self, ChatResult};
use async_chat::FromClient;
use async_std::io;
use async_std::net;
use async_std::prelude::*;

fn parse_command(line: &str) -> Option<FromClient> {
    let (command, rest) = get_next_token(line)?;

    if command == "post" {
        let (group, rest) = get_next_token(rest)?;
        let message = rest.trim_start().to_string();
        return Some(FromClient::Post {
            group_name: Arc::new(group.to_string()),
            message: Arc::new(message),
        });
    } else if command == "join" {
        let (group, rest) = get_next_token(rest)?;
        if !rest.trim_start().is_empty() {
            return None;
        }
        return Some(FromClient::Join {
            group_name: Arc::new(group.to_string()),
        });
    } else {
        eprint!("不明なコマンドです: {:?}", line);
        return None;
    }
}

fn get_next_token(mut input: &str) -> Option<(&str, &str)> {
    input = input.trim_start();

    if input.is_empty() {
        return None;
    }

    match input.find(char::is_whitespace) {
        Some(space) => Some((&input[0..space], &input[space..])),
        None => Some((input, "")),
    }
}

// メッセージ送信ロジック
async fn send_commands(mut to_server: net::TcpStream) -> ChatResult<()> {
    println!(
        "コマンド:\n\
        チャット参加: join グループ名\n\
        メッセージ送信: post グループ名 メッセージ...\n\
        Ctrl+D (ユニックス系) または Ctrl+Z (Windows) \
        接続を切断."
    );
    let mut commands_lines = io::BufReader::new(io::stdin()).lines();
    while let Some(command_result) = commands_lines.next().await {
        let command = command_result?;
        let request = match parse_command(&command) {
            Some(request) => request,
            None => continue,
        };

        utils::send_as_json(&mut to_server, &request).await?;
        to_server.flush().await?;
    }
    Ok(())
}

use async_chat::FromServer;

// 受信メッセージ表示ロジック
async fn handle_replies(from_server: net::TcpStream) -> ChatResult<()> {
    let buffered = io::BufReader::new(from_server);
    let mut reply_stream = utils::receive_as_json(buffered);

    while let Some(reply) = reply_stream.next().await {
        match reply? {
            FromServer::Message {
                group_name,
                message,
            } => {
                println!("{}にメッセージ'{}'を送信.", group_name, message);
            }
            FromServer::Error(message) => {
                println!("サーバーエラー: {}", message);
            }
        }
    }

    Ok(())
}

use async_std::task;

fn main() -> ChatResult<()> {
    let address = std::env::args()
        .nth(1)
        .expect("使用方法: client アドレス:ポート");

    task::block_on(async {
        let socket = net::TcpStream::connect(address).await?;
        socket.set_nodelay(true)?;

        let to_server = send_commands(socket.clone());
        let from_server = handle_replies(socket);

        from_server.race(to_server).await?;
        Ok(())
    })
}
