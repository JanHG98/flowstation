use tetra_core::tetra_entities::TetraEntity;
use tetra_core::Sap;
use tetra_saps::common::{SleepMode, TlmcScanState};
use tetra_saps::ltpd::LtpdMleActivityReq;
use tetra_saps::tlmc::TlmcConfigureReq;
use tetra_saps::{SapMsg, SapMsgInner};

#[test]
fn tlmc_primitive_is_routable_without_untyped_payload() {
    let message = SapMsg::new(
        Sap::TlmcSap,
        TetraEntity::Mle,
        TetraEntity::Umac,
        SapMsgInner::TlmcConfigureReq(TlmcConfigureReq::default()),
    );

    assert_eq!(*message.get_sap(), Sap::TlmcSap);
    assert_eq!(*message.get_source(), TetraEntity::Mle);
    assert_eq!(*message.get_dest(), TetraEntity::Umac);
    assert!(matches!(message.msg, SapMsgInner::TlmcConfigureReq(_)));
}

#[test]
fn ltpd_primitive_is_routable_without_untyped_payload() {
    let message = SapMsg::new(
        Sap::TlpdSap,
        TetraEntity::Sndcp,
        TetraEntity::Mle,
        SapMsgInner::LtpdMleActivityReq(LtpdMleActivityReq {
            sleep_mode: SleepMode::StayAlive,
        }),
    );

    assert_eq!(*message.get_sap(), Sap::TlpdSap);
    assert_eq!(*message.get_source(), TetraEntity::Sndcp);
    assert_eq!(*message.get_dest(), TetraEntity::Mle);
    assert!(matches!(message.msg, SapMsgInner::LtpdMleActivityReq(_)));
}

#[test]
fn sap_display_has_a_non_panicking_fallback_for_new_variants() {
    let inner = SapMsgInner::TlmcConfigureReq(TlmcConfigureReq::default());
    let rendered = inner.to_string();
    assert!(rendered.contains("TlmcConfigureReq"));
}

#[test]
fn explicit_foundation_state_defaults_to_idle() {
    assert_eq!(TlmcScanState::default(), TlmcScanState::Idle);
}
