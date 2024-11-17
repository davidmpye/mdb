#![no_std]

pub mod coin_acceptor;

    use enumn::N;
    
    const MDB_TIMEOUT_MS:u8 = 50;

    #[derive(N)]
    pub enum MDBStatus {
        ACK = 0x00,
        NAK = 0xFF,
        RET = 0xAA,
        NoReply = 0x01,        //These aren't real vals, and should never be sent over the wire.
        ChecksumErr = 0x02,    //As above
        BufOverflow = 0x03,    //As above
        Invalid = 0x04,        //As above
    }

    pub enum MDBResponse<T, U> {
        Data(T),
        StatusMsg(U),
    }

    pub struct Mdb<T: embedded_io::Write + embedded_io::Read> {
        uart : T, //The 9 bit uart that we will use to read write MDB
        pub timer: rp2040_hal::timer::Timer,
        //Should we include other settings, eg timeout?
    }

    impl <T: embedded_io::Write + embedded_io::Read>Mdb<T> {
        pub fn new (uart:T, timer: rp2040_hal::timer::Timer) -> Self {
            Self {
                uart,
                timer,
            }
        }

        pub fn receive_response(&mut self, buf:  &mut [u8]) -> MDBResponse<usize, MDBStatus> {
            //We need a scratch buffer twice the maximum message length, because 
            //2 bytes are returned by the 9 bit uart, with the first byte holding the ninth bit val.
            let mut scratch_buf: [u8; 72] = [0x00; 72];

            let mut calculated_checksum: u8 = 0x00;

            //Implementation of timeout timer
            let start_counter_val = self.timer.get_counter_low();
            let mut offset:usize = 0;

            let mut bytes_out:usize = 0;
            let mut end_of_message = false;

            loop {
                //Check to see if timeout has been exceeded
                if self.timer.get_counter_low() >= (start_counter_val +  (1000 * MDB_TIMEOUT_MS as u32)) {
                    //Timeout exceeded.
                    return MDBResponse::StatusMsg(MDBStatus::NoReply);
                } 
                match self.uart.read(&mut scratch_buf[offset..72]) {
                    Ok(count) => {
                        //Even bytes will be the byte containing just the 9th bit.
                        let mut top_byte = true;
                        for i in scratch_buf[offset..offset + count ].iter() {
                            if top_byte {
                                if *i == 0x01 {
                                    //If 9th bit is set high, this is the last byte of the message
                                    end_of_message = true;
                                }
                                top_byte = false;
                            }
                            else {
                                //The next byte the loop will process will be the top byte again
                                top_byte = true;
                            }
                            if !end_of_message {
                                //just a regular byte
                                if buf.len() == bytes_out {
                                    defmt::debug!("Buffer too small for data received");
                                    return MDBResponse::StatusMsg(MDBStatus::BufOverflow);
                                }
                                else {
                                    //Write the byte to the supplied buffer
                                    buf[bytes_out] = *i;
                                    bytes_out += 1;
                                    //Recalculate checksum
                                    calculated_checksum = calculated_checksum.wrapping_add(*i);
                                    offset += count;
                                }
                            }
                            else {
                                //The end of message flag has been received.
                                if bytes_out == 0 {
                                    //If we have received only one byte and the EOM flag is set (ie not a normal message with a checksum),
                                    //then this should be either an ACK or NAK.
                                    let  x= MDBStatus::n(*i);
                                    match x {
                                        Some(status) => {
                                            if matches!(status, MDBStatus::ACK) || matches!(status,MDBStatus::NAK) {
                                                return MDBResponse::StatusMsg(status);                           
                                            }
                                        }
                                        None => {}
                                    }
                                    //Shouldn't have got here..
                                    defmt::debug!("Got invalid status {=u8}", *i);
                                    return MDBResponse::StatusMsg(MDBStatus::Invalid);
                                }
                                else {
                                    //This is a normal multibyte message, so we should be looking at the checksum as the last byte
                                    if *i == calculated_checksum {
                                        //Send an ACK, checksum matches
                                        self.send_status_message(MDBStatus::ACK);
                                        return MDBResponse::Data(bytes_out);
                                    } 
                                    else {
                                        //Invalid checksum
                                            defmt::debug!("Invalid checksum, expected {=u8}, got {=u8}, msg length {=u8}",calculated_checksum, *i, bytes_out as u8) ;
                                            defmt::debug!("BytesData {=[u8]:#04x}", buf[0..bytes_out]);
                                            //MDB best practices say we shouldn't send a NAK, just don't reply, which should be interpreted as same.
                                            return MDBResponse::StatusMsg(MDBStatus::ChecksumErr);
                                        }
                                    }
                                
                            }
                        }
                    },
                    Err(e) => {
                        defmt::debug!("UART rx error");
                        //Don't return though, keep trying until end of timeout
                    }
                }
            }
        }

        pub fn send_data(&mut self, msg: &[u8]) {
            //It's a normal message, so needs a checksum
            let mut checksum: u8 = 0x00;
            let mut is_first_byte = true;

            for i in msg.iter() {
                //First byte is an address byte, 9th bit high
                let prefix_byte:u8;
                if is_first_byte {
                    prefix_byte = 0x01u8;
                    is_first_byte = false;
                }
                else {
                    //Rest of message - 9th bit low.
                    prefix_byte = 0x00u8;
                }
                let _ = self.uart.write(&[prefix_byte, *i]);
                //Update checksum calculation
                checksum = checksum.wrapping_add(*i); //Note, 9th bit not included in checksum
            }
            let _ = self.uart.write(&[0x00u8, checksum]);
        }

        pub fn send_status_message(&mut self, status: MDBStatus) {
            match status {
                MDBStatus::ACK | MDBStatus::NAK | MDBStatus::RET => {
                    //Send - no checksum required, 9th bit low
                    let _ = self.uart.write(&[0x00u8, status as u8]);
                },
                _ => {
                    defmt::debug!("Attempt to send invalid MDB status message - Only ACK/RET/NAK allowed");
                },
            }
        }

        pub fn send_data_and_confirm_ack(&mut self, msg: &[u8]) -> bool {
            self.send_data(msg);
            //We supply an empty buffer as we don't want any bytes received, only a status.
            let msg = self.receive_response(&mut []);
            if let MDBResponse::StatusMsg(reply) = msg {
                if matches!(reply, MDBStatus::ACK) {
                    return true;
                }
            }
            false
        }
    }

