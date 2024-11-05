use anyhow::Result;
use reqwest::Client;
use testing::peer_connection::PeerConnectionInit;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;

#[tokio::main]
async fn main() -> Result<()> {    
    let peer_connection_init = PeerConnectionInit::new().await?;

    let sdp = Client::new().get("http://127.0.0.1:8001/offer").send().await?.error_for_status()?.text().await?;
    let sdp_offer = RTCSessionDescription::offer(sdp)?;


    let (pending_peer_connection, sdp_answer) = peer_connection_init.answer(sdp_offer).await?;
    let _ = Client::new().post("http://127.0.0.1:8001/answer").body(sdp_answer.sdp).send().await?.error_for_status()?;

    let peer_connection = pending_peer_connection.resolve().await?;

    tokio::time::sleep(std::time::Duration::from_secs(30)).await;
    peer_connection.close().await?;

    Ok(())
}