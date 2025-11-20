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

// TODO add receive trait.
