use anyhow::Result;
use peer_connection::PeerConnectionInit;
use text_io::read;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;

mod codec;
pub mod peer_connection;

// --turn-username "dfruser" --turn-credential "8LzgjGpiybdjpdMP" --turn-address "turn:aspen.co.us.aerodome.com:3478"
pub const TURN_ADDRESS: &str = "turn:turn.aspen.co.us.aerodome.com:3478";
pub const TURN_USERNAME: &str = "dfruser";
pub const TURN_CREDENTIAL: &str = "foEs8ozXouHCGWPZ";

#[tokio::main]
async fn main() -> Result<()> {
    let peer_connection_server_init = PeerConnectionInit::new().await?;
    let (pending_peer_connection_server, sdp_offer) = peer_connection_server_init.offer().await?;

    let peer_connection_client_init = PeerConnectionInit::new().await?;
    let (pending_peer_connection_client, sdp_answer) = peer_connection_client_init.answer(sdp_offer).await?;

    tokio::spawn(async move {
        let _peer_connection_client = pending_peer_connection_client.resolve().await.unwrap();
        eprintln!("client resolved");
    });

    tokio::spawn(async move {
        let _peer_connection_server = pending_peer_connection_server.resolve(sdp_answer).await.unwrap();
        eprintln!("server resolved");
    });


    tokio::time::sleep(std::time::Duration::from_secs(120)).await;

    Ok(())
}
