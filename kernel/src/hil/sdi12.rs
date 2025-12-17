use crate::hil::uart::Error;
use crate::ErrorCode;

pub trait Transmit<'a> {
    fn transmit(
        &'a self,
        buffer: &'static mut [u8],
        len: usize,
    ) -> Result<(), (ErrorCode, &'static mut [u8])>;

    fn set_transmit_client(&self, client: &'a dyn TransmitClient);
}

pub trait TransmitClient {
    fn transmitted_buffer(
        &self,
        buffer: &'static mut [u8],
        length: usize,
        status: Result<(), ErrorCode>,
    );
}

pub trait Receive<'a> {
    fn receive(
        &'a self,
        buffer: &'static mut [u8],
        max_len: usize,
    ) -> Result<(), (ErrorCode, &'static mut [u8])>;

    fn set_receive_client(&self, client: &'a dyn ReceiveClient);
}

pub trait ReceiveClient {
    fn receive_buffer(
        &self,
        buffer: &'static mut [u8],
        length: usize,
        status: Result<(), ErrorCode>,
        error: Error,
    );
}
