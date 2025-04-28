use anyhow::{Result, anyhow};
use std::{
    collections::VecDeque,
    convert::Infallible,
    env::args,
    io::{BufRead, stdin},
    process::Stdio,
};
use tracing::info;

use tokio::{
    io::AsyncWriteExt,
    process::{ChildStdin, Command},
    select,
    sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel},
};

use crate::discord::IncomingDiscordMessage;

#[inline]
fn read_stdin(sender: UnboundedSender<String>) -> Result<Infallible> {
    let mut buffer = String::new();
    let mut stdin_lock = stdin().lock();

    loop {
        buffer.clear();
        stdin_lock.read_line(&mut buffer)?;
        sender.send(buffer.clone())?;
    }
}

enum StdinMessage {
    DiscordMessage(IncomingDiscordMessage),
    UserInput(String),
}

impl StdinMessage {
    #[inline]
    fn as_string(self, tellraw_prefix: &str) -> String {
        match self {
            Self::DiscordMessage(incoming_discord_message) => {
                incoming_discord_message.create_command(tellraw_prefix)
            }
            Self::UserInput(input) => input,
        }
    }

    #[inline]
    async fn write(self, tellraw_prefix: &str, to: &mut ChildStdin) -> Result<()> {
        let as_string = self.as_string(tellraw_prefix);

        to.write_all(as_string.as_bytes()).await?;
        to.flush().await?;
        Ok(())
    }
}

#[inline]
async fn pipe_stdin(
    mut stdin: ChildStdin,
    mut stdin_receiver: UnboundedReceiver<String>,
    mut discord_message_receiver: UnboundedReceiver<IncomingDiscordMessage>,
    tellraw_prefix: String,
) -> Result<Infallible> {
    loop {
        let msg = select! {
            line = stdin_receiver.recv() => {
                StdinMessage::UserInput(line.ok_or_else(|| anyhow!("stdin sender dropped"))?)
            }
            discord_message = discord_message_receiver.recv() => {
                StdinMessage::DiscordMessage(discord_message.ok_or_else(|| anyhow!("discord message sender dropped"))?)
            }
        };

        msg.write(&tellraw_prefix, &mut stdin).await?;
    }
}

#[inline]
pub async fn launch_wrapper(
    discord_message_receiver: UnboundedReceiver<IncomingDiscordMessage>,
    tellraw_prefix: String,
) -> Result<()> {
    let mut args: VecDeque<String> = args().into_iter().skip(1).collect();
    let mut command = Command::new(
        args.pop_front()
            .ok_or_else(|| anyhow!("expected first arg to be java path"))?,
    );

    let (stdin_sender, stdin_receiver) = unbounded_channel();
    std::thread::spawn(|| read_stdin(stdin_sender));

    command
        .args(args.into_iter())
        .kill_on_drop(true) // TODO: Shutdown gracefully
        .stdin(Stdio::piped());

    info!("starting server");

    let mut child = command.spawn()?;
    let stdin = child.stdin.take();
    select! {
        _ = child.wait() => {},
        _ = pipe_stdin(
            stdin.ok_or_else(|| anyhow!("child does not have stdin"))?,
            stdin_receiver,
            discord_message_receiver,
            tellraw_prefix
        ) => {}
    };

    Ok(())
}
