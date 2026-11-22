//! Loopback-only transport. Telegram performs its normal authenticated handshake end to end.
use std::{
    io,
    net::{Ipv4Addr, SocketAddr},
    time::Duration,
};
use tokio::{
    io::copy_bidirectional,
    net::{TcpListener, TcpStream},
    sync::{mpsc, oneshot},
    task::{JoinHandle, JoinSet},
};

pub struct Relay {
    pub port: u16,
    change: mpsc::Sender<(SocketAddr, oneshot::Sender<()>)>,
    worker: JoinHandle<()>,
}
impl Relay {
    pub async fn start(preferred: u16, mut target: SocketAddr) -> io::Result<Self> {
        let listener = match TcpListener::bind((Ipv4Addr::LOCALHOST, preferred)).await {
            Ok(l) => l,
            Err(e) if preferred != 0 && e.kind() == io::ErrorKind::AddrInUse => {
                TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?
            }
            Err(e) => return Err(e),
        };
        let port = listener.local_addr()?.port();
        let (change, mut changes) = mpsc::channel::<(SocketAddr, oneshot::Sender<()>)>(1);
        let worker = tokio::spawn(async move {
            let mut clients = JoinSet::new();
            loop {
                tokio::select! {
                    biased;
                    cmd=changes.recv()=>match cmd {
                        Some((next,ack))=>{clients.shutdown().await;target=next;let _=ack.send(());}
                        None=>break,
                    },
                    _=clients.join_next(), if !clients.is_empty()=>{},
                    incoming=listener.accept()=>match incoming {
                        Ok((mut local,_)) if clients.len()<64=>{
                            clients.spawn(async move {
                                if let Ok(Ok(mut remote))=tokio::time::timeout(Duration::from_secs(8),TcpStream::connect(target)).await {
                                    let _=local.set_nodelay(true);
                                    let _=remote.set_nodelay(true);
                                    let _=copy_bidirectional(&mut local,&mut remote).await;
                                }
                            });
                        }
                        Ok(_)=>{},
                        Err(_)=>tokio::time::sleep(Duration::from_millis(100)).await,
                    }
                }
            }
            drop(listener);
            clients.shutdown().await;
        });
        Ok(Self {
            port,
            change,
            worker,
        })
    }
    pub async fn switch(&self, next: SocketAddr) -> Result<(), String> {
        let (tx, rx) = oneshot::channel();
        self.change
            .send((next, tx))
            .await
            .map_err(|_| "Прокси остановлен.")?;
        rx.await.map_err(|_| "Прокси остановлен.".into())
    }
    pub async fn close(self) {
        drop(self.change);
        let _ = self.worker.await;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    #[test]
    fn power_switch_closes_connections_and_reuses_port() {
        tauri::async_runtime::block_on(async {
            let upstream = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
            let target = upstream.local_addr().unwrap();
            let relay = Relay::start(0, target).await.unwrap();
            let port = relay.port;
            let mut client = TcpStream::connect((Ipv4Addr::LOCALHOST, port))
                .await
                .unwrap();
            let (mut server, _) = upstream.accept().await.unwrap();
            client.write_all(b"req").await.unwrap();
            let mut bytes = [0; 3];
            server.read_exact(&mut bytes).await.unwrap();
            assert_eq!(&bytes, b"req");
            server.write_all(b"res").await.unwrap();
            client.read_exact(&mut bytes).await.unwrap();
            assert_eq!(&bytes, b"res");
            relay.switch(target).await.unwrap();
            assert_eq!(client.read(&mut bytes).await.unwrap(), 0);
            let mut client = TcpStream::connect((Ipv4Addr::LOCALHOST, port))
                .await
                .unwrap();
            let (_server, _) = upstream.accept().await.unwrap();
            relay.close().await;
            assert_eq!(client.read(&mut bytes).await.unwrap(), 0);
            assert!(TcpStream::connect((Ipv4Addr::LOCALHOST, port))
                .await
                .is_err());
            let again = Relay::start(port, target).await.unwrap();
            assert_eq!(again.port, port);
            again.close().await;
        });
    }
}
