use std::borrow::Cow;

use anyhow::{Result, bail};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader, BufWriter, copy},
    net::{TcpStream, tcp::OwnedWriteHalf},
    select, spawn,
    sync::{mpsc::UnboundedReceiver, oneshot},
};
use tokio_util::task::AbortOnDropHandle;
use tracing::info;

use crate::discord::IncomingDiscordMessage;

const LOGIN: i32 = 3;
const COMMAND: i32 = 2;

#[inline]
async fn skip_int<R: AsyncRead + Unpin>(mut reader: R) -> Result<()> {
    copy(&mut (&mut reader).take(4), &mut tokio::io::sink()).await?;

    Ok(())
}

struct Packet<'a> {
    pub request_id: i32,
    pub request_type: i32,
    pub payload: Cow<'a, str>,
}

impl<'a> Packet<'a> {
    #[inline]
    pub async fn read<R: AsyncRead + Unpin>(mut reader: R) -> Result<Self> {
        skip_int(&mut reader).await?;

        let request_id = reader.read_i32_le().await?;
        let request_type = reader.read_i32_le().await?;
        let mut payload_body = Vec::new();

        loop {
            let byte = reader.read_u8().await?;

            if byte == b'\0' {
                break;
            }

            if payload_body.len() == 4096 {
                bail!("payload body too large");
            }

            payload_body.push(byte);
        }

        reader.read_u8().await?; // additional padding? what?

        Ok(Self {
            request_id,
            request_type,
            payload: Cow::Owned(String::from_utf8(payload_body)?),
        })
    }

    #[inline]
    pub async fn write<W: AsyncWrite + Unpin>(&self, mut writer: W) -> Result<()> {
        let mut request_body = Vec::new();

        request_body.write_i32_le(self.request_id).await?;
        request_body.write_i32_le(self.request_type).await?;
        request_body.extend(self.payload.as_bytes());
        request_body.extend(&[0, 0]); // nul terminator and extra byte of padding

        writer.write_i32_le(request_body.len().try_into()?).await?;
        writer.write_all(&request_body).await?;

        Ok(())
    }
}

#[derive(Debug)]
pub struct RconClient {
    writer: BufWriter<OwnedWriteHalf>,
    _read_handle: AbortOnDropHandle<Result<()>>,
    request_id: i32,
    read_death_receiver: oneshot::Receiver<()>,
}

impl RconClient {
    #[inline]
    pub async fn new(host: &str, pass: &str) -> Result<Self> {
        let stream = TcpStream::connect(host).await?;
        stream.set_nodelay(true)?;
        let (reader, writer) = stream.into_split();
        let (mut reader, mut writer) = (BufReader::new(reader), BufWriter::new(writer));

        Packet {
            request_id: 0,
            request_type: LOGIN,
            payload: Cow::Borrowed(&pass),
        }
        .write(&mut writer)
        .await?;

        writer.flush().await?;

        if Packet::read(&mut reader).await?.request_id == -1 {
            bail!("incorrect password")
        }

        info!("logged into rcon");

        let (read_death_sender, read_death_receiver) = oneshot::channel();

        Ok(Self {
            writer,
            _read_handle: AbortOnDropHandle::new(spawn(async move {
                copy(&mut reader, &mut tokio::io::sink()).await?;
                let _ = read_death_sender.send(());
                Ok(())
            })),
            request_id: 1,
            read_death_receiver,
        })
    }

    #[inline]
    pub async fn handle(
        mut self,
        mut discord_message_receiver: UnboundedReceiver<IncomingDiscordMessage>,
        tellraw_prefix: String,
    ) -> Result<()> {
        loop {
            let msg = select! {
                msg = discord_message_receiver.recv() => {
                    msg
                },
                _ = &mut self.read_death_receiver => {
                    None
                }
            };

            let Some(msg) = msg else {
                bail!("discord died or rcon read died")
            };

            Packet {
                request_id: self.request_id,
                request_type: COMMAND,
                payload: Cow::Borrowed(&msg.create_command(&tellraw_prefix)),
            }
            .write(&mut self.writer)
            .await?;

            self.writer.flush().await?;
            self.request_id += 1;
        }
    }
}
