use webrtc::{api::media_engine::MIME_TYPE_VP8, rtp_transceiver::{rtp_codec::{RTCRtpCodecCapability, RTCRtpCodecParameters}, RTCPFeedback}};

pub enum Codec {
    VP8,
}

impl From<Codec> for RTCRtpCodecParameters {
    fn from(codec: Codec) -> Self {
        match codec {
            Codec::VP8 => RTCRtpCodecParameters {
                payload_type: 96,
                capability: RTCRtpCodecCapability::from(codec),
                ..Default::default()
            },
        }
    }
}

impl From<Codec> for RTCRtpCodecCapability {
    fn from(codec: Codec) -> Self {
        let common_rtcp_feedback = vec![
            RTCPFeedback {
                typ: "goog-remb".to_owned(),
                parameter: "".to_owned(),
            },
            RTCPFeedback {
                typ: "ccm".to_owned(),
                parameter: "fir".to_owned(),
            },
            RTCPFeedback {
                typ: "nack".to_owned(),
                parameter: "".to_owned(),
            },
            RTCPFeedback {
                typ: "nack".to_owned(),
                parameter: "pli".to_owned(),
            },
        ];

        match codec {
            Codec::VP8 => RTCRtpCodecCapability {
                mime_type: MIME_TYPE_VP8.to_owned(),
                clock_rate: 90000,
                channels: 0,
                rtcp_feedback: common_rtcp_feedback,
                sdp_fmtp_line: "profile-id=0".to_owned(),
            },
        }
    }
}