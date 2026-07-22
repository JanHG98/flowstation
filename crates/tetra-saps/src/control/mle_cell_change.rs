//! Internal control messages for the infrastructure-side MLE cell-change runtime.
//!
//! These commands are deliberately local control-plane messages. They are not
//! ETSI air-interface PDUs and must not become the future Edge/Core wire format.

use tetra_core::{BitBuffer, TetraAddress};

use crate::lcmc::fields::chan_alloc_req::CmceChanAllocReq;

use crate::common::{
    CellIdentity, MleChannelCommandValid, MleChannelRequestReason,
    MleChannelRequestRetryDelay, MleChannelResponseType, MleFailCause,
};

#[derive(Debug, Clone)]
pub enum MleCellChangeControl {
    GrantPrepare {
        subscriber: TetraAddress,
        command: MleChannelCommandValid,
        target_cell: Option<CellIdentity>,
        mm_sdu: Option<BitBuffer>,
    },
    RejectPrepare {
        subscriber: TetraAddress,
        cause: MleFailCause,
        mm_sdu: Option<BitBuffer>,
    },
    AcknowledgeRestore {
        subscriber: TetraAddress,
        cmce_sdu: BitBuffer,
        /// Optional local traffic-channel allocation accompanying D-RESTORE-ACK.
        chan_alloc: Option<CmceChanAllocReq>,
    },
    RejectRestore {
        subscriber: TetraAddress,
        cause: MleFailCause,
    },
    RespondChannelRequest {
        subscriber: TetraAddress,
        response: MleChannelResponseType,
        reason: MleChannelRequestReason,
        retry_delay: MleChannelRequestRetryDelay,
    },
}
