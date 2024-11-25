use core::fmt::Display;

use crate::MDBResponse;
use crate::MDBStatus;
use crate::Mdb;

use defmt::Format;
use embedded_hal::delay::DelayNs;
use enumn::N;


const RESET:u8 = 0x10;

const SETUP_PREFIX:u8 = 0x11;
const SETUP_CONFIG_DATA:u8 = 0x00;
const SETUP_MAX_MIN_PRICES:u8 = 0x01;
const SETUP_REPLY_READER_CONFIG_DATA:u8 = 0x01;

const POLL_CMD:u8 = 0x12;
//Various poll replies
const POLL_REPLY_JUST_RESET:u8 = 0x00;
const POLL_REPLY_READER_CONFIG_DATA:u8 = 0x01;
const POLL_REPLY_DISPLAY_REQUEST:u8 = 0x02;
const POLL_REPLY_BEGIN_SESSION:u8 = 0x03;
const POLL_REPLY_SESSION_CANCEL_REQUEST:u8 = 0x04;
const POLL_REPLY_VEND_APPROVED:u8 = 0x05;
const POLL_REPLY_VEND_DENIED:u8 = 0x06;
const POLL_REPLY_END_SESSION:u8 = 0x07;
const POLL_REPLY_CANCELLED:u8 = 0x08;
const POLL_REPLY_PERIPHERAL_ID:u8 = 0x09;
const POLL_REPLY_MALFUNCTION:u8 = 0x0A;
const POLL_REPLY_OUT_OF_SEQUENCE:u8 = 0x0B;

const POLL_REPLY_REVALUE_APPROVED:u8 = 0x0F;
const POLL_REPLY_REVALUE_DENIED:u8 = 0x0F;
const POLL_REPLY_REVALUE_LIMIT_AMOUNT:u8 = 0x0F;
const POLL_REPLY_USER_FILE_DATA:u8 = 0x10;
const POLL_REPLY_TIME_DATE_REQUEST:u8 = 0x11;
const POLL_REPLY_DATA_ENTRY_REQUEST:u8 = 0x12;
const POLL_REQUEST_DATA_ENTRY_CANCEL:u8 = 0x13;
//We do not support FTL
const POLL_REPLY_DIAGNOSTICS:u8 = 0xFF;

//Vend commands
const VEND_PREFIX:u8 = 0x13;
const VEND_REQUEST:u8 = 0x00;
const VEND_CANCEL:u8 = 0x01;
const VEND_SUCCESS:u8 = 0x02;
const VEND_FAILURE:u8 = 0x03;
const VEND_SESSION_COMPLETE:u8 = 0x04;
const VEND_CASH_SALE = 0x05;
const NEGATIVE_VEND_REQUEST:u8 = 0x06;
//Vend replies
const VEND_REPLY_APPROVED:u8 = 0x05;
const VEND_REPLY_DENIED:u8 = 0x06;
const VEND_REPLY_END_SESSION:u8 = 0x07;
const VEND_REPLY_CANCELLED:u8 = 0x08;

//Vend reader commands
const VEND_READER_PREFIX:u8 = 0x14;
const VEND_READER_DISABLE:u8 = 0x00;
const VEND_READER_ENABLE:u8 = 0x01;
const VEND_READER_CANCEL:u8 = 0x02;
const VEND_READER_DATA_ENTRY_RESP = 0x03;

//Vend revalue commands
const VEND_REVALUE_PREFIX:u8 = 0x15;
const VEND_REVALUE_REQUEST:u8 = 0x00;
const VEND_REVALUE_LIMIT_REQUEST:u8 = 0x01;
//Vend revalue replies
const VEND_REPLY_REVALUE_APPROVED:u8 = 0x0D;
const VEND_REPLY_REVALUE_DENIED:u8 = 0x0E;
const VEND_REPLY_REVALUE_LIMIT_AMOUNT:u8 = 0x0F;

pub enum CashlessDeviceFeatureLevel {
    Level1,
    Level2,
    Level3,
}

pub struct CashlessDevice {
    pub feature_level: CashlessDeviceFeatureLevel,
    pub country_code: u16,
    pub scale_factor: u8,
    pub decimal_places: u8,
    pub max_response_time: u8,
    pub misc_options: u8,
}

impl CashlessDevice {
    
    pub fn init<T: embedded_io::Write + embedded_io::Read>(bus: &mut Mdb<T>) -> Option<Self> {

    }
    
}