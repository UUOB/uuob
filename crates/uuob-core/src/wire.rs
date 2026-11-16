//! Length-prefixed postcard frames over QUIC streams.

use anyhow::{Context, bail};
use iroh::endpoint::{RecvStream, SendStream};
use serde::{Serialize, de::DeserializeOwned};

const MAX_FRAME: usize = 64 * 1024 * 1024;

pub async fn write_frame<T: Serialize>(send: &mut SendStream, msg: &T) -> anyhow::Result<()> {
    let bytes = postcard::to_allocvec(msg)?;
    send.write_all(&(bytes.len() as u32).to_be_bytes()).await?;
    send.write_all(&bytes).await?;
    Ok(())
}

pub async fn read_frame<T: DeserializeOwned>(recv: &mut RecvStream) -> anyhow::Result<T> {
    let mut len = [0u8; 4];
    recv.read_exact(&mut len).await.context("read frame length")?;
    let len = u32::from_be_bytes(len) as usize;
    if len > MAX_FRAME {
        bail!("frame too large: {len}");
    }
    let mut buf = vec![0u8; len];
    recv.read_exact(&mut buf).await.context("read frame body")?;
    Ok(postcard::from_bytes(&buf)?)
}
