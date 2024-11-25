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
const VEND_CASH_SALE:u8 = 0x05;
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
const VEND_READER_DATA_ENTRY_RESP:u8 = 0x03;

//Vend revalue commands
const VEND_REVALUE_PREFIX:u8 = 0x15;
const VEND_REVALUE_REQUEST:u8 = 0x00;
const VEND_REVALUE_LIMIT_REQUEST:u8 = 0x01;
//Vend revalue replies
const VEND_REPLY_REVALUE_APPROVED:u8 = 0x0D;
const VEND_REPLY_REVALUE_DENIED:u8 = 0x0E;
const VEND_REPLY_REVALUE_LIMIT_AMOUNT:u8 = 0x0F;

#[derive(Format)]
pub enum CashlessDeviceFeatureLevel {
    Level1,
    Level2,
    Level3,
}

#[derive(Format)]
pub struct CashlessDevice {
    pub feature_level: CashlessDeviceFeatureLevel,
    pub country_code: u16,
    pub scale_factor: u8,
    pub decimal_places: u8,
    pub max_response_time: u8,
    pub can_restore_funds : bool,
    pub multivend_capable: bool,
    pub has_display: bool,
    pub supports_cash_sale_cmd : bool,

    //These come back from the peripheral ID command (0x09)
    pub manufacturer_code: [u8;2],
    pub serial_number: [u8; 11],
    pub model_number: [u8;11],
    pub software_version: [u8;2],

    //Level 3 features
    pub supports_ftl: bool,
    pub monetary_format_32_bit: bool,
    pub supports_multicurrency: bool,
    pub supports_negative_vend: bool,
    pub supports_data_entry: bool,
    pub supports_always_idle: bool,
}

impl CashlessDevice {
    /// Given the first byte of the poll command, this function will
    /// return its' length.  Needed in order to tokenize multiple 
    /// responses to a poll command when they are chained into a single message
    pub fn poll_response_length(&self, poll_cmd: u8) -> u8 {
        match poll_cmd {
            POLL_REPLY_JUST_RESET => 8,
            POLL_REPLY_DISPLAY_REQUEST => 34,
            POLL_REPLY_BEGIN_SESSION => {
                match self.feature_level {
                    CashlessDeviceFeatureLevel::Level1 => 3,
                    _ => 10,
                    //Would be 17 if expanded currency mode enabled, but not supported currently
                }
            },
            POLL_REPLY_SESSION_CANCEL_REQUEST => 1,
            POLL_REPLY_VEND_APPROVED  => 5,
            POLL_REPLY_VEND_DENIED => 1,
            POLL_REPLY_END_SESSION => 1,
            POLL_REPLY_CANCELLED => 1,
            POLL_REPLY_PERIPHERAL_ID => {
                match self.feature_level {
                    //Because the library identifies as L3 VMC, the L3 device will give us the option bits
                    //making its' reply 34 bytes long
                    CashlessDeviceFeatureLevel::Level3 => 34, 
                    _ => 30,
                }
            },
            POLL_REPLY_MALFUNCTION => 2,
            POLL_REPLY_OUT_OF_SEQUENCE => {
                match self.feature_level {
                    CashlessDeviceFeatureLevel::Level1 => 1, 
                    _ => 2
                }
            },
            POLL_REPLY_REVALUE_APPROVED => 1,
            POLL_REPLY_REVALUE_DENIED => 1,
            POLL_REPLY_REVALUE_LIMIT_AMOUNT => 3,
            POLL_REPLY_TIME_DATE_REQUEST => 1,
            POLL_REPLY_DATA_ENTRY_REQUEST => 2,
        }
    }

    pub fn init<T: embedded_io::Write + embedded_io::Read>(bus: &mut Mdb<T>) -> Option<Self> {

    }

}