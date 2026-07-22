use tetra_core::BitBuffer;
use tetra_pdus::mm::enums::location_update_type::LocationUpdateType;
use tetra_pdus::mm::enums::reject_cause::RejectCause;
use tetra_pdus::mm::pdus::d_location_update_proceeding::DLocationUpdateProceeding;
use tetra_pdus::mm::pdus::d_location_update_reject::DLocationUpdateReject;

#[test]
fn location_update_proceeding_roundtrip_preserves_vassi_and_home_mni() {
    let pdu = DLocationUpdateProceeding {
        ssi: 0xE0_0042,
        address_extension: 0x0400_01,
        proprietary: None,
    };
    let mut encoded = BitBuffer::new_autoexpand(16);
    pdu.to_bitbuf(&mut encoded).unwrap();
    encoded.seek(0);
    let decoded = DLocationUpdateProceeding::from_bitbuf(&mut encoded).unwrap();
    assert_eq!(decoded, pdu);
    assert_eq!(encoded.get_len_remaining(), 0);
}

#[test]
fn location_update_reject_parser_handles_cipher_off_and_on() {
    for (cipher_control, ciphering_parameters) in [(false, None), (true, Some(0x155))] {
        let pdu = DLocationUpdateReject {
            location_update_type: LocationUpdateType::MigratingLocationUpdating,
            reject_cause: RejectCause::MessageConsistencyError as u8,
            cipher_control,
            ciphering_parameters,
            address_extension: Some(0x0400_01),
            cell_type_control: None,
            proprietary: None,
        };
        let mut encoded = BitBuffer::new_autoexpand(24);
        pdu.to_bitbuf(&mut encoded).unwrap();
        encoded.seek(0);
        let decoded = DLocationUpdateReject::from_bitbuf(&mut encoded).unwrap();
        assert_eq!(decoded, pdu);
        assert_eq!(encoded.get_len_remaining(), 0);
    }
}
