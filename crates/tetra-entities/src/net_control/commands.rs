// ---------------------------------------------------------------------------
// Command / CommandResponse — concrete enums sent through the channel
//
// The command server sends a Command; the stack processes it and returns
// a CommandResponse.  Placeholder variants are provided for now.
// ---------------------------------------------------------------------------

use bitcode::{Decode, Encode};
use serde::{Deserialize, Serialize};


#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, Serialize, Deserialize)]
pub enum MobilityClientState {
    Unknown,
    Attached,
    Detached,
}

#[derive(Debug, Clone, Encode, Decode, Serialize, Deserialize)]
pub struct MobilityClassOfMs {
    pub freq_simplex_duplex: bool,
    pub multislot_phase_mod: bool,
    pub concurrent_multicarrier: bool,
    pub voice: bool,
    pub e2e_encryption_not_supported: bool,
    pub circuit_mode_data: bool,
    pub tetra_packet_data: bool,
    pub fast_switching: bool,
    pub dck_encryption: bool,
    pub clch_needed: bool,
    pub concurrent_circuit_mode: bool,
    pub original_advanced_link: bool,
    pub minimum_mode: bool,
    pub carrier_specific_signalling: bool,
    pub authentication: bool,
    pub sck_encryption: bool,
    pub air_interface_version: u8,
    pub common_scch: bool,
    pub reserved_21: bool,
    pub mac_d_blck: bool,
    pub extended_advanced_link: bool,
    pub d8psk: bool,
}

#[derive(Debug, Clone, Encode, Decode, Serialize, Deserialize)]
pub struct MobilityContextPayload {
    pub home_issi: u32,
    pub state: MobilityClientState,
    pub groups: Vec<u32>,
    pub energy_saving_mode: u8,
    pub monitoring_frame: Option<u8>,
    pub monitoring_multiframe: Option<u8>,
    pub class_of_ms: Option<MobilityClassOfMs>,
    pub last_handle: u32,
    pub tei: Option<u64>,
}


#[derive(Debug, Clone, Encode, Decode, Serialize, Deserialize)]
pub struct GroupPolicyDefinition {
    pub gssi: u32,
    pub enabled: bool,
    pub attach_allowed: bool,
    pub dgna_allowed: bool,
    pub call_allowed: bool,
    pub sds_allowed: bool,
    pub emergency_allowed: bool,
    pub call_priority: u8,
    pub class_of_usage: u8,
}

#[derive(Debug, Clone, Encode, Decode, Serialize, Deserialize)]
pub struct GroupMembershipPolicy {
    pub issi: u32,
    pub gssi: u32,
    pub allowed: bool,
    pub auto_attach: bool,
    pub locked: bool,
}

/// Command received from the remote command server.
#[derive(Debug, Clone, Encode, Decode, Serialize, Deserialize)]
pub enum ControlCommand {
    /// Send an SDS for local delivery
    SendSds {
        handle: u32,
        source_ssi: u32,
        dest_ssi: u32,
        dest_is_group: bool,
        len_bits: u16,
        payload: Vec<u8>,
    },

    /// Send an already-built SDS Type-4 payload for local delivery.
    SendRawSdsType4 {
        handle: u32,
        source_ssi: u32,
        dest_ssi: u32,
        dest_is_group: bool,
        len_bits: u16,
        payload: Vec<u8>,
    },

    /// Forcibly deregister a terminal from the BS
    KickMs { issi: u32 },

    /// Dynamic Group Number Assignment (SS-DGNA, ETSI EN 300 392-2 §16).
    ///
    /// BS-initiated: attach (or detach) a single GSSI on an already-registered
    /// terminal over the air, by sending it an unsolicited D-ATTACH/DETACH GROUP
    /// IDENTITY. Local-only — no Brew propagation is performed for this command.
    Dgna {
        /// Target terminal (must be registered on the cell).
        issi: u32,
        /// Group to assign/remove.
        gssi: u32,
        /// `true` = assign/attach the group, `false` = deassign/detach it.
        attach: bool,
    },

    /// Restart the FlowStation service (systemctl restart tetra)
    RestartService,

    /// Stop the FlowStation service (systemctl stop tetra)
    ShutdownService,

    /// Add a live SDS message to the broadcast queue.
    /// The message will be transmitted to all MSs on the cell at the next HMD interval,
    /// round-robining with the static Home Mode Display text.
    /// `repeat_count = 0` means repeat indefinitely; `> 0` auto-removes after N transmissions.
    AddLiveSds {
        text: String,
        protocol_id: u8,
        source_issi: u32,
        repeat_count: u32,
    },

    /// Remove a live SDS message from the queue by its ID.
    DeleteLiveSds { id: u32 },

    /// Remove all live SDS messages from the queue.
    ClearLiveSds,

    /// Operator-clear an active emergency for one ISSI (`issi == 0` clears all). Local-only;
    /// clears the source session so a subsequent emergency re-send raises a fresh alarm.
    ClearEmergency { issi: u32 },

    /// Export the local MM context of a registered subscriber for transfer to another TBS.
    MobilityExportContext { handle: u32, issi: u32 },

    /// Import a context that was exported by another TBS.
    MobilityImportContext {
        handle: u32,
        local_issi: u32,
        context: MobilityContextPayload,
    },

    /// Remove a transferred context from the source TBS after the target confirmed import.
    MobilityRemoveContext {
        handle: u32,
        issi: u32,
        reason: String,
    },

    /// Apply the centrally managed subscriber admission policy to this TBS.
    ///
    /// `allow_all = true` opens the cell regardless of `allowed_issis`.  With
    /// `allow_all = false`, only the listed Home ISSIs may register.  An empty
    /// list in closed mode therefore means deny-all, unlike the legacy local
    /// dashboard whitelist where an empty list means open network.
    SubscriberAccessPolicyApply {
        handle: u32,
        revision: u64,
        allow_all: bool,
        allowed_issis: Vec<u32>,
        disconnect_unauthorized: bool,
    },


    /// Apply centrally managed group definitions and membership policy.
    GroupAccessPolicyApply {
        handle: u32,
        revision: u64,
        allow_unlisted_groups: bool,
        enforce_memberships: bool,
        reconcile_registered: bool,
        groups: Vec<GroupPolicyDefinition>,
        memberships: Vec<GroupMembershipPolicy>,
    },

    /// Execute one centrally coordinated DGNA operation and return an explicit result.
    GroupDgnaApply {
        handle: u32,
        issi: u32,
        gssi: u32,
        attach: bool,
        force: bool,
    },

    /// Placeholder command A.
    CommandA { handle: u32, parameter: u32 },
    /// Placeholder command B.
    TestCmdB {
        handle: u32,
        source_ssi: u32,
        is_group: bool,
        payload: Vec<u8>,
    },
}

/// Response sent back after processing a [`ControlCommand`].
#[derive(Debug, Clone, Encode, Decode, Serialize, Deserialize)]
pub enum ControlResponse {
    CommandAResponse { handle: u32, result: u32 },
    SendSdsResponse { handle: u32, success: bool },
    KickMsResponse { issi: u32, success: bool },
    MobilityContextExported {
        handle: u32,
        issi: u32,
        found: bool,
        context: Option<MobilityContextPayload>,
        message: String,
    },
    MobilityContextImported {
        handle: u32,
        local_issi: u32,
        success: bool,
        message: String,
    },
    MobilityContextRemoved {
        handle: u32,
        issi: u32,
        success: bool,
        message: String,
    },

    GroupAccessPolicyApplied {
        handle: u32,
        revision: u64,
        success: bool,
        group_count: u32,
        membership_count: u32,
        attached_count: u32,
        detached_count: u32,
        message: String,
    },
    GroupDgnaApplied {
        handle: u32,
        issi: u32,
        gssi: u32,
        attach: bool,
        success: bool,
        message: String,
    },
    SubscriberAccessPolicyApplied {
        handle: u32,
        revision: u64,
        success: bool,
        allow_all: bool,
        allowed_count: u32,
        disconnected_count: u32,
        message: String,
    },
}
