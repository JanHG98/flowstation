use crossbeam_channel::Sender;

use tetra_config::bluestation::SharedConfig;
use tetra_core::tetra_entities::TetraEntity;
use tetra_core::{BitBuffer, BurstType, PhyBlockNum, PhyBlockType, Sap, TdmaTime, TrainingSequence, unimplemented_log};
use tetra_pdus::phy::traits::rxtx_dev::RxBurstBits;
use tetra_pdus::phy::traits::rxtx_dev::{RxTxDev, TxSlotBits};
use tetra_saps::tp::TpUnitdataInd;
use tetra_saps::{SapMsg, SapMsgInner};

use crate::phy::components::phy_io_file::{FileWriteMsg, PhyIoFileMode};
use crate::phy::components::{burst_consts::*, slotter, train_consts::*};
use crate::umac::subcomp::bs_sched::MACSCHED_TX_AHEAD;
use crate::{MessageQueue, TetraEntityTrait};

use super::components::phy_io_file::PhyIoFile;

pub struct PhyBs<D: RxTxDev> {
    config: SharedConfig,
    dltime: TdmaTime,

    /// Channel for asynchronous downlink TX data logging
    dl_tx_sender: Option<Sender<FileWriteMsg>>,
    /// Channel for asynchronous uplink RX data logging
    ul_rx_sender: Option<Sender<FileWriteMsg>>,

    /// Testing mode: Transmit input data from file instead of from stack
    dl_input_file: Option<PhyIoFile>,
    /// Testing mode: Parse input data from file instead of from SDR
    ul_input_file: Option<PhyIoFile>,

    /// RX/TX device
    rxtxdev: D,

    tick: u64,
}

impl<D: RxTxDev> PhyBs<D> {
    pub fn new(config: SharedConfig, rxtxdev: D) -> Self {
        let c = &config.config().phy_io;

        // Create async writers for file logging of generated DL and received UL signals
        let dl_tx_logger = c
            .dl_tx_file
            .as_ref()
            .and_then(|f| PhyIoFile::create_async_writer(f, "dl_tx_logger".to_string()).ok());
        let ul_rx_logger = c
            .ul_rx_file
            .as_ref()
            .and_then(|f| PhyIoFile::create_async_writer(f, "ul_rx_logger".to_string()).ok());

        // Open input files overriding either generated DL or received UL data
        let dl_input_file = if let Some(ref f) = c.dl_input_file {
            Some(PhyIoFile::new(f, PhyIoFileMode::ReadRepeat).expect("Failed to open dl_input_file"))
        } else {
            None
        };
        let ul_input_file = if let Some(ref f) = c.ul_input_file {
            Some(PhyIoFile::new(f, PhyIoFileMode::Read).expect("Failed to open ul_input_file"))
        } else {
            None
        };

        Self {
            config,
            dltime: TdmaTime::default(), // updated in tick_start
            dl_tx_sender: dl_tx_logger,
            ul_rx_sender: ul_rx_logger,
            dl_input_file,
            ul_input_file,
            rxtxdev,
            tick: 0,
        }
    }

    fn send_rxblock_to_lmac(
        queue: &mut MessageQueue,
        carrier_num: u16,
        train_type: TrainingSequence,
        burst_type: BurstType,
        block_type: PhyBlockType,
        block_num: PhyBlockNum,
        bits: BitBuffer,
        rssi_dbfs: f32,
    ) {
        // Uplink timeslot is two after downlink. Thus was transmitted at dltime - 2
        let sapmsg = SapMsg {
            sap: Sap::TpSap,
            src: TetraEntity::Phy,
            dest: TetraEntity::Lmac,
            msg: SapMsgInner::TpUnitdataInd(TpUnitdataInd {
                carrier_num,
                train_type,
                burst_type,
                block_type,
                block_num,
                block: bits,
                rssi_dbfs,
            }),
        };
        queue.push_back(sapmsg);
    }

    fn split_rxslot_and_send_to_lmac(queue: &mut MessageQueue, carrier_num: u16, burst: &RxBurstBits<'_>) {
        let train_seq = burst.train_type;
        match train_seq {
            TrainingSequence::NormalTrainSeq1 => {
                // burst.bits is a variable-length slice from the demodulator. A length
                // mismatch (DSP glitch, misconfiguration) would otherwise panic on the
                // slice index below — drop and log instead so the cell survives.
                if burst.bits.len() != NUB_BITS {
                    tracing::warn!("PHY: NUB burst wrong length ({} != {}), dropping", burst.bits.len(), NUB_BITS);
                    return;
                }

                let mut blk = BitBuffer::new(NUB_BLK_BITS * 2);
                blk.copy_bits_from_bitarr(&burst.bits[NUB_BLK1_OFFSET..NUB_BLK1_OFFSET + NUB_BLK_BITS]);
                blk.copy_bits_from_bitarr(&burst.bits[NUB_BLK2_OFFSET..NUB_BLK2_OFFSET + NUB_BLK_BITS]);
                blk.seek(0);

                Self::send_rxblock_to_lmac(
                    queue,
                    carrier_num,
                    train_seq,
                    BurstType::NUB,
                    PhyBlockType::NUB,
                    PhyBlockNum::Both,
                    blk,
                    burst.rssi_dbfs,
                );
            }

            TrainingSequence::NormalTrainSeq2 => {
                if burst.bits.len() != NUB_BITS {
                    tracing::warn!("PHY: NUB burst wrong length ({} != {}), dropping", burst.bits.len(), NUB_BITS);
                    return;
                }

                let blk1 = BitBuffer::from_bitarr(&burst.bits[NUB_BLK1_OFFSET..NUB_BLK1_OFFSET + NUB_BLK_BITS]);
                let blk2 = BitBuffer::from_bitarr(&burst.bits[NUB_BLK2_OFFSET..NUB_BLK2_OFFSET + NUB_BLK_BITS]);

                Self::send_rxblock_to_lmac(
                    queue,
                    carrier_num,
                    train_seq,
                    BurstType::NUB,
                    PhyBlockType::NUB,
                    PhyBlockNum::Block1,
                    blk1,
                    burst.rssi_dbfs,
                );
                Self::send_rxblock_to_lmac(
                    queue,
                    carrier_num,
                    train_seq,
                    BurstType::NUB,
                    PhyBlockType::NUB,
                    PhyBlockNum::Block2,
                    blk2,
                    burst.rssi_dbfs,
                );
            }
            TrainingSequence::ExtendedTrainSeq => {
                if burst.bits.len() != CUB_BITS {
                    tracing::warn!("PHY: CUB burst wrong length ({} != {}), dropping", burst.bits.len(), CUB_BITS);
                    return;
                }

                let mut blk = BitBuffer::new(CUB_BLK_BITS * 2);
                blk.copy_bits_from_bitarr(&burst.bits[CUB_BLK1_OFFSET..CUB_BLK1_OFFSET + CUB_BLK_BITS]);
                blk.copy_bits_from_bitarr(&burst.bits[CUB_BLK2_OFFSET..CUB_BLK2_OFFSET + CUB_BLK_BITS]);
                blk.seek(0);

                Self::send_rxblock_to_lmac(
                    queue,
                    carrier_num,
                    train_seq,
                    BurstType::CUB,
                    PhyBlockType::SSN1,
                    PhyBlockNum::Block1,
                    blk,
                    burst.rssi_dbfs,
                );
            }

            // SyncTrainSeq, NormalTrainSeq3 and NotFound are not handled here (sync bursts
            // are processed elsewhere; NotFound is filtered by the caller). A real demod
            // can legitimately classify a burst as SyncTrainSeq, so this must NOT be an
            // unreachable!()/panic — drop and log instead.
            other => {
                tracing::debug!("PHY: training sequence {:?} not handled in split_rxslot, dropping", other);
            }
        }
    }


    fn build_dl_burst(&mut self, prim: tetra_saps::tp::TpUnitdataReqSlot) -> Option<[u8; TIMESLOT_TYPE4_BITS]> {
        let mut dl_burst = [0u8; TIMESLOT_TYPE4_BITS];
        if let Some(dl_input_file) = &mut self.dl_input_file {
            dl_input_file.read_block(&mut dl_burst).expect("Failed to read dl_input_file data");
            return Some(dl_burst);
        }

        let Some(mut bbk_bits) = prim.bbk else {
            tracing::warn!("PHY: TP slot missing BBK, dropping");
            return None;
        };
        let mut bbk = [0u8; 30];
        bbk_bits.to_bitarr(&mut bbk);

        Some(match prim.burst_type {
            BurstType::SDB => {
                if prim.train_type != TrainingSequence::SyncTrainSeq || prim.blk1.is_none() || prim.blk2.is_none() {
                    tracing::warn!("PHY: invalid SDB slot, dropping");
                    return None;
                }

                let mut blk1 = [0u8; 120];
                let mut blk2 = [0u8; 216];
                prim.blk1.expect("SDB missing blk1").to_bitarr(&mut blk1);
                prim.blk2.expect("SDB missing blk2").to_bitarr(&mut blk2);
                slotter::build_sdb(&blk1, &bbk, &blk2)
            }
            BurstType::NDB => {
                let mut blk1 = [0u8; 216];
                let mut blk2 = [0u8; 216];

                match prim.train_type {
                    TrainingSequence::NormalTrainSeq1 => {
                        let Some(mut blk1_src) = prim.blk1 else {
                            tracing::warn!("PHY: NDB/NormalTrainSeq1 missing blk1, dropping");
                            return None;
                        };
                        if prim.blk2.is_some() {
                            tracing::warn!("PHY: NDB/NormalTrainSeq1 has unexpected blk2, ignoring blk2");
                        }
                        blk1_src.to_bitarr(&mut blk1);
                        blk1_src.to_bitarr(&mut blk2);
                    }
                    TrainingSequence::NormalTrainSeq2 => {
                        let Some(mut blk1_src) = prim.blk1 else {
                            tracing::warn!("PHY: NDB/NormalTrainSeq2 missing blk1, dropping");
                            return None;
                        };
                        let Some(mut blk2_src) = prim.blk2 else {
                            tracing::warn!("PHY: NDB/NormalTrainSeq2 missing blk2, dropping");
                            return None;
                        };
                        blk1_src.to_bitarr(&mut blk1);
                        blk2_src.to_bitarr(&mut blk2);
                    }
                    other => {
                        tracing::warn!("PHY: unsupported training sequence {:?} for NDB burst, dropping", other);
                        return None;
                    }
                }

                slotter::build_ndb(prim.train_type, &blk1, &bbk, &blk2)
            }
            other => {
                tracing::warn!("PHY: unsupported burst type {:?}, dropping", other);
                return None;
            }
        })
    }

    fn rx_tpsap_prim(&mut self, queue: &mut MessageQueue, message: SapMsg) {
        self.tick += 1;

        let prims = match message.msg {
            SapMsgInner::TpUnitdataReq(prim) => vec![prim],
            SapMsgInner::TpUnitdataReqSlots(batch) => batch.slots,
            _ => {
                tracing::error!("BUG: unexpected message or state -- routing error");
                return;
            }
        };

        let mut dl_bursts = Vec::with_capacity(prims.len());
        let mut carrier_nums = Vec::with_capacity(prims.len());
        for prim in prims {
            let carrier_num = prim.carrier_num;
            if let Some(burst) = self.build_dl_burst(prim) {
                carrier_nums.push(carrier_num);
                dl_bursts.push(burst);
            }
        }

        let tx_time = self.dltime.add_timeslots(MACSCHED_TX_AHEAD as i32);
        let tx_slots = dl_bursts
            .iter()
            .zip(carrier_nums.iter())
            .map(|(burst, carrier_num)| TxSlotBits {
                carrier_num: *carrier_num,
                time: tx_time,
                slot: Some(burst),
                ..Default::default()
            })
            .collect::<Vec<_>>();

        if let Some(dl_tx_sender) = &self.dl_tx_sender {
            for burst in &dl_bursts {
                let _ = dl_tx_sender.try_send(FileWriteMsg::WriteBlock(burst.to_vec()));
            }
        }

        let rx = self.rxtxdev.rxtx_timeslot(&tx_slots).expect("Got error from rxtx_timeslot");

        // With adjacent dual carriers, the secondary-channel demodulator can sometimes
        // lock onto the same uplink burst that was already decoded on the primary
        // carrier. Forwarding both copies makes UMAC/LLC see duplicate MAC-ACCESS,
        // duplicate ACKs and duplicated fragment starts, which can put radios into a
        // setup/retry loop. Collect all burst candidates for this TDMA slot and drop
        // exact duplicate bitstreams seen in the same full/subslot, keeping the
        // strongest RSSI copy. Real independent traffic on two carriers has different
        // bits and is still forwarded normally.
        #[derive(Debug)]
        struct RxBurstCandidate {
            carrier_num: u16,
            subslot_id: u8, // 3 = fullslot/file header id, 1/2 = subslot
            train_type: TrainingSequence,
            rssi_dbfs: f32,
            bits: Vec<u8>,
        }

        let mut candidates: Vec<RxBurstCandidate> = Vec::new();
        for rx_slot in rx {
            let Some(rx_slot) = rx_slot else {
                continue;
            };

            let mut push_candidate = |subslot_id: u8, label: &'static str, burst: &RxBurstBits<'_>| {
                if burst.train_type == TrainingSequence::NotFound {
                    return;
                }

                tracing::info!(
                    ts=%self.dltime,
                    carrier=rx_slot.carrier_num,
                    "rx_tpsap_prim got {:?} in {}",
                    burst.train_type,
                    label
                );

                if let Some(existing) = candidates.iter_mut().find(|c| {
                    c.subslot_id == subslot_id && c.train_type == burst.train_type && c.bits.as_slice() == burst.bits
                }) {
                    if burst.rssi_dbfs > existing.rssi_dbfs {
                        tracing::debug!(
                            ts=%self.dltime,
                            from_carrier=existing.carrier_num,
                            to_carrier=rx_slot.carrier_num,
                            old_rssi=existing.rssi_dbfs,
                            new_rssi=burst.rssi_dbfs,
                            "PHY: duplicate RX burst on adjacent carrier; keeping stronger copy"
                        );
                        existing.carrier_num = rx_slot.carrier_num;
                        existing.rssi_dbfs = burst.rssi_dbfs;
                    } else {
                        tracing::debug!(
                            ts=%self.dltime,
                            kept_carrier=existing.carrier_num,
                            dropped_carrier=rx_slot.carrier_num,
                            kept_rssi=existing.rssi_dbfs,
                            dropped_rssi=burst.rssi_dbfs,
                            "PHY: dropping duplicate RX burst from adjacent carrier"
                        );
                    }
                    return;
                }

                candidates.push(RxBurstCandidate {
                    carrier_num: rx_slot.carrier_num,
                    subslot_id,
                    train_type: burst.train_type,
                    rssi_dbfs: burst.rssi_dbfs,
                    bits: burst.bits.to_vec(),
                });
            };

            push_candidate(3, "fullslot", &rx_slot.slot);
            push_candidate(1, "subslot1", &rx_slot.subslot1);
            push_candidate(2, "subslot2", &rx_slot.subslot2);
        }

        for candidate in candidates {
            if let Some(ul_rx_sender) = &self.ul_rx_sender {
                let _ = ul_rx_sender.try_send(FileWriteMsg::WriteHeaderAndBlock(
                    candidate.subslot_id,
                    self.tick,
                    candidate.bits.clone(),
                ));
            }

            let burst = RxBurstBits {
                train_type: candidate.train_type,
                bits: &candidate.bits,
                rssi_dbfs: candidate.rssi_dbfs,
            };
            Self::split_rxslot_and_send_to_lmac(queue, candidate.carrier_num, &burst);
        }
    }

    fn rx_tpc_prim(&mut self, _queue: &mut MessageQueue, _message: SapMsg) {
        // TPC SAP not implemented yet. Log instead of crashing the PHY worker.
        unimplemented_log!("rx_tpc_prim: TPC SAP not implemented");
    }
}

impl<D: RxTxDev + Send + 'static> TetraEntityTrait for PhyBs<D> {
    fn entity(&self) -> TetraEntity {
        TetraEntity::Phy
    }

    fn rx_prim(&mut self, queue: &mut MessageQueue, message: SapMsg) {
        tracing::debug!("rx_prim: {:?}", message);
        // tracing::debug!(ts=%message.dltime, "rx_prim: {:?}", message);

        match message.sap {
            Sap::TpSap => {
                self.rx_tpsap_prim(queue, message);
            }
            Sap::TpcSap => {
                self.rx_tpc_prim(queue, message);
            }
            _ => {
                tracing::error!("BUG: unexpected message or state -- routing error");
                return;
            }
        }
    }

    fn tick_start(&mut self, _queue: &mut MessageQueue, ts: TdmaTime) {
        self.dltime = ts;
    }
}
