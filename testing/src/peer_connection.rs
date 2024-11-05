use std::sync::Arc;

use anyhow::anyhow;
use anyhow::Result;
use shadow_clone::shadow_clone;
use tokio::sync::{oneshot, Mutex};
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::ice_transport::ice_gatherer_state::RTCIceGathererState;
use webrtc::ice_transport::ice_gathering_state::RTCIceGatheringState;
use webrtc::interceptor::registry::Registry;
use webrtc::{
    api::{media_engine::MediaEngine, APIBuilder},
    ice_transport::ice_server::RTCIceServer,
    peer_connection::{
        configuration::RTCConfiguration, peer_connection_state::RTCPeerConnectionState,
        sdp::session_description::RTCSessionDescription, RTCPeerConnection,
    },
    rtp_transceiver::{
        rtp_codec::RTPCodecType, rtp_transceiver_direction::RTCRtpTransceiverDirection,
        RTCRtpTransceiverInit,
    },
};

use crate::codec::Codec;
use crate::TURN_ADDRESS;
use crate::TURN_CREDENTIAL;
use crate::TURN_USERNAME;

pub struct PeerConnectionInit {
    peer_connection: RTCPeerConnection,
    open_rx: oneshot::Receiver<Result<()>>
}

impl PeerConnectionInit {
    pub async fn new() -> Result<Self> {
        let peer_connection = Self::create_base_peer_connection().await?;

        Self::enable_data_channels(&peer_connection).await?;
        Self::register_video_transceiver(&peer_connection).await?;
        let open_rx = Self::register_open_handler(&peer_connection);

        Ok(Self { peer_connection, open_rx })
    }

    async fn create_base_peer_connection() -> Result<RTCPeerConnection> {
        let (media_engine, interceptors) = Self::get_media_engine()?;
        let ice_servers = Self::get_ice_servers();

        let peer_connection = APIBuilder::new()
            .with_media_engine(media_engine)
            .with_interceptor_registry(interceptors)
            .build()
            .new_peer_connection(RTCConfiguration {
                ice_servers,
                ..Default::default()
            })
            .await?;

        Ok(peer_connection)
    }

    fn get_media_engine() -> Result<(MediaEngine, Registry)> {
        let mut media_engine = MediaEngine::default();
        media_engine.register_codec(Codec::VP8.into(), RTPCodecType::Video)?;

        let mut interceptors = Registry::new();
        interceptors = register_default_interceptors(interceptors, &mut media_engine)?;

        Ok((media_engine, interceptors))
    }

    fn get_ice_servers() -> Vec<RTCIceServer> {
        // let stun_server = RTCIceServer {
        //     urls: vec![
        //         "stun:stun.l.google.com:19302".to_owned(),
        //         "stun:stun1.l.google.com:19302".to_owned(),
        //         "stun:stun2.l.google.com:19302".to_owned(),
        //         "stun:stun3.l.google.com:19302".to_owned(),
        //         "stun:stun4.l.google.com:19302".to_owned(),
        //     ],
        //     ..Default::default()
        // };
    
        let turn_server = RTCIceServer {
            urls: vec![TURN_ADDRESS.to_owned()],
            username: TURN_USERNAME.to_string(),
            credential: TURN_CREDENTIAL.to_string(),
        };

        vec![turn_server]
    }

    async fn register_video_transceiver(peer_connection: &RTCPeerConnection) -> Result<()> {
        peer_connection
            .add_transceiver_from_kind(
                RTPCodecType::Video,
                Some(RTCRtpTransceiverInit {
                    direction: RTCRtpTransceiverDirection::Sendrecv,
                    send_encodings: vec![],
                }),
            )
            .await?;

        Ok(())
    }

    async fn enable_data_channels(peer_connection: &RTCPeerConnection) -> Result<()> {
        // @TODO: Test if data channel creation triggers renegotiation
        // create a dummy data channel to enable the negotiation of an SCTP transport
        // for a WebRTC connection a single SCTP transport multiplexes all data channels
        //
        // if no data channel is in the initial SDP offer/answer then one won't be established
        // which means data channels created after the fact will never be able to connect
        peer_connection.create_data_channel("dummy", None).await?;

        Ok(())
    }

    fn register_open_handler(peer_connection: &RTCPeerConnection) -> oneshot::Receiver<Result<()>> {
        let (established_tx, established_rx) = oneshot::channel();
        let established_tx = Arc::new(Mutex::new(Some(established_tx)));
        
        peer_connection.on_peer_connection_state_change(Box::new({
            shadow_clone!(established_tx);
            move |state| {
                shadow_clone!(established_tx);
                Box::pin(async move {
                    let connection_result = match state {
                        // success state
                        RTCPeerConnectionState::Connected => Some(Ok(())),
    
                        // failure states
                        RTCPeerConnectionState::Disconnected => Some(Err(anyhow!("disconnected"))),
                        RTCPeerConnectionState::Failed => Some(Err(anyhow!("failed"))),
                        RTCPeerConnectionState::Closed => Some(Err(anyhow!("closed"))),
    
                        // intermediary states (ignore)
                        RTCPeerConnectionState::New
                        | RTCPeerConnectionState::Connecting => None,
                        
                        RTCPeerConnectionState::Unspecified => {
                            tracing::warn!("reached unspecified connection state");
                            None
                        },
                    };
    
                    if let Some(connection_result) = connection_result {
                        match established_tx.lock().await.take() {
                            Some(established_tx) => {
                               let _ = established_tx.send(connection_result);
                            }
                            None => {
                                tracing::warn!("establishment handler ran twice before being overwritten")
                            }
                        }
                    }
                })
            }
        }));

        established_rx
    }

    fn register_ice_gathering_handler(peer_connection: &RTCPeerConnection) -> oneshot::Receiver<()> {
        let (gathering_complete_tx, gathering_complete_rx) = oneshot::channel();
        let gathering_complete_tx = Arc::new(Mutex::new(Some(gathering_complete_tx)));

        peer_connection.on_ice_gathering_state_change(Box::new(move |state| {
            shadow_clone!(gathering_complete_tx);
            Box::pin({
                shadow_clone!(gathering_complete_tx);

                async move { 
                    if matches!(state, RTCIceGathererState::Complete) {
                        match gathering_complete_tx.lock().await.take() {
                            Some(gathering_complete_tx) => {
                               let _ = gathering_complete_tx.send(());
                            }
                            None => {
                                tracing::warn!("establishment handler ran twice before being overwritten")
                            }
                        }
                    }
                }
            })
        }));

        gathering_complete_rx
    }

    fn unregister_open_handler(peer_connection: &RTCPeerConnection) {
        peer_connection.on_peer_connection_state_change(Box::new(|state| Box::pin(async move { 
            tracing::debug!(?state, "post open peer connection state change")
        })));
    }

    pub(self) async fn resolve(self) -> Result<PeerConnection> {
        self.open_rx.await??;
        Self::unregister_open_handler(&self.peer_connection);

        Ok(PeerConnection { peer_connection: self.peer_connection })
    }

    pub async fn offer(self) -> Result<(PendingPeerConnectionOffer, RTCSessionDescription)> {
        let ice_complete = Self::register_ice_gathering_handler(&self.peer_connection);
        
        // create and set an initial sdp offer to start ICE gathering
        let sdp_offer_init = self.peer_connection.create_offer(None).await?;
        self.peer_connection
            .set_local_description(sdp_offer_init)
            .await?;

        // await ice gathering to complete before retreiving the finished offer
        ice_complete.await?;

        let sdp_offer = self.peer_connection
            .local_description()
            .await
            .ok_or(anyhow!("failed to set local description"))?;

        Ok((PendingPeerConnectionOffer { peer_connection_init: self }, sdp_offer))
    }

    pub async fn answer(self, offer: RTCSessionDescription) -> Result<(PendingPeerConnectionAnswer, RTCSessionDescription)> {
        let ice_complete = Self::register_ice_gathering_handler(&self.peer_connection);
        
        // set the remote offer
        self.peer_connection.set_remote_description(offer).await?;
        
        // create and set an initial sdp answer to start ICE gathering
        let sdp_answer_init = self.peer_connection.create_answer(None).await?;
        self.peer_connection
            .set_local_description(sdp_answer_init)
            .await?;

        // await ice gathering to complete before retreiving the finished answer
        ice_complete.await?;

        let sdp_answer = self.peer_connection
            .local_description()
            .await
            .ok_or(anyhow!("failed to set local description"))?;

        Ok((PendingPeerConnectionAnswer { peer_connection_init: self }, sdp_answer))
    }
}

pub struct PendingPeerConnectionOffer {
    peer_connection_init: PeerConnectionInit,
}

impl PendingPeerConnectionOffer {
    pub async fn resolve(self, answer: RTCSessionDescription) -> Result<PeerConnection> {
        self.peer_connection_init.peer_connection.set_remote_description(answer).await?;
        
        self.peer_connection_init.resolve().await
    }
}

pub struct PendingPeerConnectionAnswer {
    peer_connection_init: PeerConnectionInit,
}

impl PendingPeerConnectionAnswer {
    pub async fn resolve(self) -> Result<PeerConnection> {
        self.peer_connection_init.resolve().await
    }
}

pub struct PeerConnection {
    peer_connection: RTCPeerConnection,
}

impl std::ops::Deref for PeerConnection {
    type Target = RTCPeerConnection;

    fn deref(&self) -> &Self::Target {
        &self.peer_connection
    }
}