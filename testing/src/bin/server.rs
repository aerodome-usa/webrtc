use std::{convert::Infallible, sync::Arc};

use anyhow::{anyhow, Result};
use bytes::Bytes;
use futures::TryFutureExt;
use testing::peer_connection::{PeerConnectionInit, PendingPeerConnectionOffer};
use tokio:: sync::Mutex;
use warp::{filters::body, http::StatusCode, reject::reject, reply, Filter};
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;

macro_rules! with_clone {
    ($state:ident) => {
        warp::any().map({ let this = $state.clone(); move || this.clone() })
    };
}

#[tokio::main]
async fn main() -> Result<()> {
    console_subscriber::init();
    
    let pending_connection = Arc::new(Mutex::new(None));

    let offer = warp::path("offer")
        .and(with_clone!(pending_connection))
        .and_then(|pending_connections: Arc<Mutex<Option<PendingPeerConnectionOffer>>>| async move {
            let inner = async move {

                let peer_connection_init = PeerConnectionInit::new().await?;
                let (pending_connection, sdp_offer) = peer_connection_init.offer().await?;

                pending_connections.lock().await.replace(pending_connection);

                Ok::<_, anyhow::Error>(sdp_offer.sdp)
            };

            inner.map_err(|_| reject()).await
        });

    let answer = warp::path("answer")
        .and(with_clone!(pending_connection))  
        .and(body::bytes())
        .and_then(|pending_connection: Arc<Mutex<Option<PendingPeerConnectionOffer>>>, body: Bytes| async move {
            let inner = async move {
                let pending_connection = pending_connection.lock().await.take().ok_or(anyhow!("not present"))?;

                let sdp = String::from_utf8(body.to_vec())?;
                let sdp_answer = RTCSessionDescription::answer(sdp)?;

                let peer_connection = pending_connection.resolve(sdp_answer).await?;

                tokio::spawn(async move {
                    eprintln!("new peer connection");
                    tokio::time::sleep(std::time::Duration::from_secs(60)).await;

                    eprintln!("{}", peer_connection.connection_state());
                    let _ = peer_connection.close().await;
                });

                Ok::<_, anyhow::Error>("")
            };

            inner.map_err(|_| reject()).await
        });
    
    let routes = offer.or(answer).recover(|_rejection| async move { Ok::<_, Infallible>(reply::with_status("", StatusCode::INTERNAL_SERVER_ERROR)) });
    
    warp::serve(routes).run(([127, 0, 0, 1], 8001)).await;

    Ok(())
}