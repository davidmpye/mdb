use core::fmt::Display;

use crate::MDBResponse;
use crate::MDBStatus;
use crate::Mdb;

//use super::{self as mdb, MDBStatus};

use defmt::Format;
use embedded_hal::delay::DelayNs;
use enumn::N;
use rp2040_hal::clocks::ClockGate;

//All coin acceptors should support these commands
const RESET_CMD: u8 = 0x08;
const SETUP_CMD: u8 = 0x09;
const TUBE_STATUS_CMD: u8 = 0x0A;
const POLL_CMD: u8 = 0x0B;
const COIN_TYPE_CMD: u8 = 0x0C;
const DISPENSE_CMD: u8 = 0x0D;

//Level 3 'expansion' commands all start with 0x0F
const L3_CMD_PREFIX: u8 = 0x0F;

//These should only be sent to a coin acceptor that identifies as supporting L3
const L3_IDENT_CMD: u8 = 0x00;
const L3_FEATURE_ENABLE_CMD: u8 = 0x01;
const L3_PAYOUT_CMD: u8 = 0x02;
const L3_PAYOUT_STATUS_CMD: u8 = 0x03;
const L3_PAYOUT_VALUE_POLL_CMD: u8 = 0x04;
const L3_DIAG_CMD: u8 = 0x05;

#[derive(Copy, Clone, Format)]
pub struct CoinType {
    pub unscaled_value:u16,
    pub routeable_to_tube:bool,
    pub tube_full:bool,
    pub num_coins: u8,
}

#[derive(Copy, Clone, Format, N)]
pub enum ChangerStatus {
    EscrowPressed = 0x01,
    ChangerPayoutBusy = 0x02,
    NoCredit = 0x03,
    DefectiveTubeSensor = 0x04,
    DoubleArrival = 0x05,
    AcceptorUnplugged = 0x06,
    TubeJam = 0x07,
    RomChecksumError = 0x08,
    CoinRoutingError = 0x09,
    ChangerBusy = 0x10,
    ChangerWasReset = 0x11,
    CoinJam = 0x12,
    PossibleCoinRemoval = 0x13,
}

#[derive(Copy, Clone, Format)]
pub enum L3ChangerStatus {
    PoweringUp,
    PoweringDown,
    Ok,
    KeypadShifted,
    ManualFillOrPayoutActive,
    NewInventoryInfoAvailable,
    InhibitedByVmc,
    GeneralError(GeneralErrorSubtype), //Subcode attached
    DiscriminatorError(DiscriminatorErrorSubtype), //Subcode attached
    AcceptGateError(AcceptGateErrorSubtype),
    SeparatorError(SeparatorModuleErrorSubtype),
    DispenserError, //Only non-specific (0x00) defined, so no point!
    CoinCassetteError(CoinCassetteErrorSubtype),
}

#[derive(Copy, Clone, Format, N)]
pub enum GeneralErrorSubtype {
    NonSpecific = 0x00,
    Cksum1 = 0x01,
    Cksum2 = 0x02,
    LowLineVoltage = 0x03,
}

#[derive(Copy, Clone, Format, N)]
pub enum DiscriminatorErrorSubtype {
    NonSpecific = 0x00,
    FlightDeckOpen = 0x10,
    EscrowReturnStuck = 0x11,
    CoinJam = 0x30,
    DiscriminationBelowStandard = 0x41,
    ValSensorAErr = 0x50,
    ValSensorBErr = 0x51,
    ValSensorCErr = 0x52,
    TempExceeded = 0x53,
    OpticsFailure = 0x54,
}

#[derive(Copy, Clone, Format, N)]
pub enum AcceptGateErrorSubtype {
    NonSpecific = 0x00,
    CoinsDidNotExit = 0x30,
    GateAlarm = 0x31,
    GateOpeNNoCoin = 0x40,
    PostGateSensorCovered = 0x50,
}

#[derive(Copy, Clone, Format, N)]
pub enum SeparatorModuleErrorSubtype {
    NonSpecific = 0x00,
    SortSensor = 0x10,
}

#[derive(Copy, Clone, Format, N)]
pub enum CoinCassetteErrorSubtype {
    NonSpecific = 0x00,
    CassetteRemoved = 0x02,
    CashBoxSensorError = 0x03,
    SunlightOnSensors = 0x04,
}

#[derive(Copy, Clone)]
pub struct CoinInsertedEvent {
    pub coin_type: u8,        //What number coin it is
    pub unscaled_value: u16,  //Unscaled value
    pub routing: CoinRouting, //where it was routed to
    pub coins_remaining: u8,  //what the coin acceptor thinks the tube count now is
}

#[derive(Copy, Clone)]
pub struct ManualDispenseEvent {
    pub coin_type: u8,       //type of the coin
    pub unscaled_value: u16, //unscaled value
    pub number: u8,          //Number of coins dispensed
    pub coins_remaining: u8, //Remaining coins
}

//A poll event might be one of the following:
#[derive(Copy, Clone)]
pub enum PollEvent {
    //Slugs inserted since last poll
    SlugCount(u8),
    Status(ChangerStatus),
    Coin(CoinInsertedEvent),
    ManualDispense(ManualDispenseEvent),
}

#[derive(Format, Copy, Clone)]
pub enum CoinRouting {
    CashBox,
    Tube,
    Reject,
    Unknown,
}

#[derive(Format)]
pub struct CoinAcceptor {
    pub feature_level: CoinAcceptorLevel,
    pub country_code: [u8; 2],
    pub scaling_factor: u8,
    pub decimal_places: u8,
    pub coin_routing: [u8; 2],
    pub coin_type_credit: [u8; 16],
    pub l3_features: Option<CoinAcceptorL3Features>,
}

#[derive(Format)]
pub struct CoinAcceptorL3Features {
    pub manufacturer_code: [u8; 3],
    pub serial_number: [u8; 12],
    pub model: [u8; 12],
    pub software_ver: [u8; 2],
    pub optional_features: [Option<OptionalFeature>; 4],
}

#[derive(Format)]
pub enum CoinAcceptorLevel {
    Level2,
    Level3,
}

#[derive(Format)]
pub enum OptionalFeature {
    AlternativePayoutSupported,
    ExtendedDiagnosticCmdSupported,
    ControlledManualFillAndPayoutSupported,
    FileTransferLayerSupported,
}

impl CoinAcceptor {
    pub fn init<T: embedded_io::Write + embedded_io::Read>(bus: &mut Mdb<T>) -> Option<Self> {
        //Start with a reset
        bus.send_data(&[RESET_CMD]);

        //Give it 100mS to get over its' reset
        bus.timer.delay_ms(100);

        //Now send a setup command
        bus.send_data(&[SETUP_CMD]);

        let mut buf: [u8; 72] = [0x00; 72];
        if let MDBResponse::Data(size) = bus.receive_response(&mut buf) {
            if size != 23 {
                defmt::debug!("Error - coin acceptor init received incorrect byte count");
                return None;
            }
            let mut coinacceptor = CoinAcceptor {
                feature_level: match buf[0] {
                    0x02 => CoinAcceptorLevel::Level2,
                    0x03 => CoinAcceptorLevel::Level3,
                    _ => {
                        defmt::debug!("Coin acceptor reported unknown feature level - assuming L2");
                        CoinAcceptorLevel::Level2
                    }
                },
                country_code: buf[1..3].try_into().unwrap(),
                scaling_factor: buf[3],
                decimal_places: buf[4],
                coin_routing: buf[5..7].try_into().unwrap(),
                coin_type_credit: buf[7..23].try_into().unwrap(),
                l3_features: None,
            };

            defmt::debug!("Initial coin acceptor discovery complete");
            //If this is a level 3 coin acceptor, we need to discover its' level 3 features here
            if matches!(coinacceptor.feature_level, CoinAcceptorLevel::Level3) {
                defmt::debug!("Probing L3 features");
                //interrogate Level 3 dispensers to discover device details and features supported
                bus.send_data(&[L3_CMD_PREFIX, L3_IDENT_CMD]);

                let mut features_to_enable: u8 = 0x00;

                if let MDBResponse::Data(size) = bus.receive_response(&mut buf) {
                    if size != 33 {
                        defmt::debug!(
                            "Coin acceptor L3 identify command received wrong length reply"
                        );
                    } else {
                        let l3 = CoinAcceptorL3Features {
                            manufacturer_code: buf[0..3].try_into().unwrap(),
                            serial_number: buf[3..15].try_into().unwrap(),
                            model: buf[15..27].try_into().unwrap(),
                            software_ver: buf[27..29].try_into().unwrap(),

                            //Parse the optional feature byte
                            optional_features: {
                                let mut features = [None, None, None, None];
                                let mut feature_count = 0;
                                if buf[32] & 0x01 == 0x01 {
                                    features[feature_count] =
                                        Some(OptionalFeature::AlternativePayoutSupported);
                                    feature_count += 1;
                                    //We want to enable this if it is supported
                                    features_to_enable |= 0x01;
                                };
                                if buf[32] & 0x02 == 0x02 {
                                    features[feature_count] =
                                        Some(OptionalFeature::ExtendedDiagnosticCmdSupported);
                                    feature_count += 1;
                                    //We want to enable this if it is supported
                                    features_to_enable |= 0x02;
                                };
                                if buf[32] & 0x04 == 0x04 {
                                    features[feature_count] = Some(
                                        OptionalFeature::ControlledManualFillAndPayoutSupported,
                                    );
                                    feature_count += 1;
                                };
                                if buf[32] & 0x08 == 0x08 {
                                    features[feature_count] =
                                        Some(OptionalFeature::FileTransferLayerSupported);
                                    feature_count += 1;
                                };
                                features
                            },
                        };
                        coinacceptor.l3_features = Some(l3);

                        //If it supports Alt Payout and ExtendedDiags we want to enable those.
                        if bus.send_data_and_confirm_ack(&[
                            L3_CMD_PREFIX,
                            L3_FEATURE_ENABLE_CMD,
                            0x00,
                            0x00,
                            0x00,
                            features_to_enable,
                        ]) {
                            defmt::debug!(
                                "Desired L3 features enabled - flag {=u8:#x}",
                                features_to_enable
                            );
                        } else {
                            defmt::debug!("Failed to enable desired L3 features");
                        }
                    }
                }
            }
            return Some(coinacceptor);
        }
        return None;
    }

    pub fn enable_coins<T: embedded_io::Write + embedded_io::Read>(
        &mut self,
        bus: &mut Mdb<T>,
        coin_mask: u16,
    ) -> bool {
        //Which coins you want to enable - NB We enable manual dispense for all coins automatically.
        bus.send_data_and_confirm_ack(&[
            COIN_TYPE_CMD,
            (coin_mask & 0xFF) as u8,
            ((coin_mask >> 8) & 0xFF) as u8,
            0xFF,
            0xFF,
        ])
    }

    pub fn l3_request_payout<T: embedded_io::Write + embedded_io::Read>(
        &mut self,
        bus: &mut Mdb<T>,
        credit: u16,
    ) -> bool {
        //Scale the payout amount by the coin reader's acceptor amount
        let credit_scaled = credit / self.scaling_factor as u16;

        defmt::debug!(
            "Attempting to pay out scaled amount of {=u8}",
            credit_scaled as u8
        );
        if credit_scaled > 255 {
            //We cannot pay out this much credit in one go....!
            defmt::debug!("Unable to pay out this much credit - exceeds max amount (amount/scaling factor >255)");
            return false;
        };

        defmt::debug!("Sending payout L3 cmd as {=u8}", credit_scaled as u8);
        bus.send_data_and_confirm_ack(&[L3_CMD_PREFIX, L3_PAYOUT_CMD, credit_scaled as u8])
    }

    pub fn poll<T: embedded_io::Write + embedded_io::Read>(
        &mut self,
        bus: &mut Mdb<T>,
    ) -> [Option<PollEvent>; 16] {
        //You might get up to 16 poll events and you should process them in order..
        let mut poll_results: [Option<PollEvent>; 16] = [None; 16];
        let mut result_count: usize = 0;

        //Send poll command
        bus.send_data(&[POLL_CMD]);

        //Read poll response - max 16 bytes
        let mut buf: [u8; 16] = [0x00; 16];
        let poll_response = bus.receive_response(&mut buf);
        //Parse response
        match poll_response {
            MDBResponse::StatusMsg(status) => {
                if matches!(status, MDBStatus::ACK) {
                    //nothing to report;
                }
            }
            MDBResponse::Data(count) => {
                //small state machine to handle 2 byte nature of potential messages.
                enum ParseState {
                    ManualDispense(u8),
                    CoinDeposited(u8),
                    NoState,
                }
                let mut state: ParseState = ParseState::NoState;

                for byte in &buf[0..count] {
                    match state {
                        ParseState::NoState => {
                            if byte & 0x80 == 0x80 {
                                //Enter manual dispense paree, and wait for byte 2 to arrive
                                state = ParseState::ManualDispense(*byte);
                            } else if byte & 0x40 == 0x40 {
                                //Enter coin deposited state, and wait for byte 2 to arrive
                                state = ParseState::CoinDeposited(*byte);
                            } else if byte & 0x20 == 0x20 {
                                //FYI: Slugs are 'items' not recognised as valid coins
                                //US English term apparently - eg a washer to try to fool the acceptor.
                                poll_results[result_count] =
                                    Some(PollEvent::SlugCount(byte & 0x1F));
                                result_count += 1;
                            } else {
                                match ChangerStatus::n(*byte) {
                                    Some(status) => {
                                        poll_results[result_count] =
                                            Some(PollEvent::Status(status));
                                        result_count += 1;
                                    }
                                    None => {
                                        defmt::debug!("Unrecognised status byte received in poll")
                                    }
                                }
                            };
                        }
                        ParseState::CoinDeposited(b) => {
                            ////Someone has deposited a coin
                            poll_results[result_count] = Some(PollEvent::Coin(CoinInsertedEvent {
                                coin_type: b & 0x0F,
                                unscaled_value: self.coin_type_credit[(b & 0x0F) as usize] as u16
                                    * self.scaling_factor as u16,
                                routing: {
                                    match b & 0x30 {
                                        0x00 => CoinRouting::CashBox,
                                        0x10 => CoinRouting::Tube,
                                        0x30 => CoinRouting::Reject,
                                        _ => {
                                            // shouldn't happen...
                                            CoinRouting::Unknown
                                        }
                                    }
                                },
                                coins_remaining: *byte,
                            }));
                            result_count += 1;

                            //Reset the state machine
                            state = ParseState::NoState;
                        }
                        ParseState::ManualDispense(b) => {
                            poll_results[result_count] =
                                Some(PollEvent::ManualDispense(ManualDispenseEvent {
                                    coin_type: b & 0x0F,
                                    unscaled_value: self.coin_type_credit[(b & 0x0F) as usize]
                                        as u16
                                        * self.scaling_factor as u16,
                                    number: (b >> 4) & 0x07,
                                    coins_remaining: *byte,
                                }));
                            result_count += 1;
                            //Reset the state machine
                            state = ParseState::NoState;
                        }
                    }
                }
            }
        }
        poll_results
    }

    pub fn l3_diagnostic_status<T: embedded_io::Write + embedded_io::Read>(
        &mut self,
        bus: &mut Mdb<T>,
    ) -> [Option<L3ChangerStatus>; 8] {
        //Fixme - we should check we are a l3 changer prior to sending this command....
        let mut statuses: [Option<L3ChangerStatus>; 8] = [None; 8];
        let mut num_statuses: usize = 0;

        bus.send_data(&[L3_CMD_PREFIX, L3_DIAG_CMD]);

        let mut buf: [u8; 16] = [0x00; 16];
        match bus.receive_response(&mut buf) {
            MDBResponse::Data(len) => {
                //Two byte statemachine for parsing
                pub enum State {
                    AwaitingFirstByte,
                    AwaitingSecondByte(u8), //u8 = firstbyte
                }
                let mut parser_state = State::AwaitingFirstByte;

                for byte in &buf[0..len] {
                    match parser_state {
                        State::AwaitingFirstByte => {
                            parser_state = State::AwaitingSecondByte(*byte);
                        }
                        State::AwaitingSecondByte(firstbyte) => {
                            //Store the status into the return array now both bytes have arrived
                            statuses[num_statuses] = match firstbyte {
                                0x01 => Some(L3ChangerStatus::PoweringUp),
                                0x02 => Some(L3ChangerStatus::PoweringDown),
                                0x03 => Some(L3ChangerStatus::Ok),
                                0x04 => Some(L3ChangerStatus::KeypadShifted),
                                0x06 => Some(L3ChangerStatus::InhibitedByVmc),
                                0x10 => {
                                    if let Some(suberror) = GeneralErrorSubtype::n(*byte) {
                                        Some(L3ChangerStatus::GeneralError(suberror))
                                    } else {
                                        defmt::debug!(
                                            "Unrecognised general error subcode {=u8}",
                                            *byte
                                        );
                                        Some(L3ChangerStatus::GeneralError(
                                            GeneralErrorSubtype::NonSpecific,
                                        ))
                                    }
                                }
                                0x11 => {
                                    if let Some(suberror) = DiscriminatorErrorSubtype::n(*byte) {
                                        Some(L3ChangerStatus::DiscriminatorError(suberror))
                                    } else {
                                        defmt::debug!(
                                            "Unrecognised discriminator error subcode {=u8}",
                                            *byte
                                        );
                                        Some(L3ChangerStatus::DiscriminatorError(
                                            DiscriminatorErrorSubtype::NonSpecific,
                                        ))
                                    }
                                }
                                0x12 => {
                                    if let Some(suberror) = AcceptGateErrorSubtype::n(*byte) {
                                        Some(L3ChangerStatus::AcceptGateError(suberror))
                                    } else {
                                        defmt::debug!(
                                            "Unrecognised accept gate error subcode {=u8}",
                                            *byte
                                        );
                                        Some(L3ChangerStatus::AcceptGateError(
                                            AcceptGateErrorSubtype::NonSpecific,
                                        ))
                                    }
                                }
                                0x13 => {
                                    if let Some(suberror) = SeparatorModuleErrorSubtype::n(*byte) {
                                        Some(L3ChangerStatus::SeparatorError(suberror))
                                    } else {
                                        defmt::debug!(
                                            "Unrecognised separator error subcode {=u8}",
                                            *byte
                                        );
                                        Some(L3ChangerStatus::SeparatorError(
                                            SeparatorModuleErrorSubtype::NonSpecific,
                                        ))
                                    }
                                }
                                0x14 => Some(L3ChangerStatus::DispenserError),
                                0x15 => {
                                    if let Some(suberror) = CoinCassetteErrorSubtype::n(*byte) {
                                        Some(L3ChangerStatus::CoinCassetteError(suberror))
                                    } else {
                                        defmt::debug!(
                                            "Unrecognised coin cassette error subcode {=u8}",
                                            *byte
                                        );
                                        Some(L3ChangerStatus::CoinCassetteError(
                                            CoinCassetteErrorSubtype::NonSpecific,
                                        ))
                                    }
                                }
                                _ => {
                                    defmt::debug!(
                                        "Unrecognised main error opcode {=u8}",
                                        firstbyte
                                    );
                                    None
                                }
                            };
                            num_statuses += 1;
                            //Reset the parser ready for the first byte of the next error code pair
                            parser_state = State::AwaitingFirstByte;
                        }
                    }
                }
            }
            MDBResponse::StatusMsg(msg) => {
                //Nothing to do - I don't think this is a valid response
            }
        }

        statuses
    }
}
