use kernel::hil::gpio::{Configure, Output};
use kernel::hil::sdi12;
use kernel::hil::time::{Alarm, AlarmClient, ConvertTicks};
use kernel::hil::uart::{self, Uart};

use kernel::hil::uart::Error;
use kernel::utilities::cells::{MapCell, OptionalCell};

use kernel::ErrorCode;

const WAKE_SENSORS_INTERVAL_MS: u32 = 13;
const MARKIING_INTERVAL_MS: u32 = 9;

use crate::gpio::{AlternateFunction, Mode};

enum Sdi12State {
    TxBreak(usize, &'static mut [u8]),
    Tx(usize, &'static mut [u8]),
    Idle,
}

pub struct Sdi12<'a, U: Uart<'a>, A: Alarm<'a>> {
    uart: &'a U,
    uart_pin: &'a crate::gpio::Pin<'a>,
    command_pin: &'a crate::gpio::Pin<'a>,
    alarm: &'a A,
    state: MapCell<Sdi12State>,
    tx_client: OptionalCell<&'a dyn sdi12::TransmitClient>,
    rx_client: OptionalCell<&'a dyn sdi12::ReceiveClient>,
}

impl<'a, U: Uart<'a>, A: Alarm<'a>> Sdi12<'a, U, A> {
    pub fn new(
        uart: &'a U,
        uart_pin: &'a crate::gpio::Pin<'a>,
        command_pin: &'a crate::gpio::Pin<'a>,
        alarm: &'a A,
    ) -> Self {
        Sdi12 {
            uart,
            uart_pin,
            command_pin,
            alarm,
            state: MapCell::new(Sdi12State::Idle),
            tx_client: OptionalCell::empty(),
            rx_client: OptionalCell::empty(),
        }
    }
}

impl<'a, U: Uart<'a>, A: Alarm<'a>> sdi12::Transmit<'a> for Sdi12<'a, U, A> {
    fn transmit(
        &self,
        data: &'static mut [u8],
        len: usize,
    ) -> Result<(), (ErrorCode, &'static mut [u8])> {
        // We unwrap here because this is undefined if the state
        // is None (e.g. should only ever be mapped).
        let state = self.state.take().unwrap();
        kernel::debug!("SDI12: Transmit requested");
        match state {
            Sdi12State::Idle => {
                kernel::debug!("SDI12: Moving to TxBreak state");
                self.state.replace(Sdi12State::TxBreak(len, data));
            }
            Sdi12State::Tx(enum_len, enum_data) => {
                // Already transmitting.
                self.state.replace(Sdi12State::Tx(enum_len, enum_data));
                return Err((ErrorCode::BUSY, data));
            }
            Sdi12State::TxBreak(stored_len, stored_data) => {
                // In a pre-transmitting state
                // Put the existing TxBreak state back and return the incoming buffer
                self.state
                    .replace(Sdi12State::TxBreak(stored_len, stored_data));
                return Err((ErrorCode::BUSY, data));
            }
        }

        // Set command pin low to prepare for transmission
        self.command_pin.make_output();
        self.command_pin.clear();

        let interval = self.alarm.ticks_from_ms(WAKE_SENSORS_INTERVAL_MS);
        self.alarm.set_alarm(self.alarm.now(), interval);

        Ok(())
    }

    fn set_transmit_client(&self, client: &'a dyn sdi12::TransmitClient) {
        self.tx_client.set(client);
    }
}

impl<'a, U: Uart<'a>, A: Alarm<'a>> sdi12::Receive<'a> for Sdi12<'a, U, A> {
    fn receive(
        &self,
        buffer: &'static mut [u8],
        max_len: usize,
    ) -> Result<(), (ErrorCode, &'static mut [u8])> {
        kernel::debug!("SDI12: Receive requested");
        match self.uart.receive_buffer(buffer, max_len) {
            Ok(()) => {
                kernel::debug!("SDI12: UART receive started successfully");
                Ok(())
            }
            Err((err, buf)) => {
                kernel::debug!("SDI12: UART receive failed with error: {:?}", err);
                Err((ErrorCode::FAIL, buf))
            }
        }
    }

    fn set_receive_client(&self, client: &'a dyn sdi12::ReceiveClient) {
        self.rx_client.set(client);
    }
}

impl<'a, U: Uart<'a>, A: Alarm<'a>> AlarmClient for Sdi12<'a, U, A> {
    fn alarm(&self) {
        // Time to hold high has elapsed. Reconfigure as alternate function
        // for USART transmission.
        self.uart_pin.set_mode(Mode::AlternateFunctionMode);
        self.uart_pin.set_alternate_function(AlternateFunction::AF7);

        let state = self.state.take().unwrap();
        match state {
            Sdi12State::Idle => {
                // Should not happen.
                unreachable!("SDI12 Alarm fired in Idle state.");
            }
            Sdi12State::TxBreak(len, data) => {
                // Time to send marking interval.
                kernel::debug!("SDI12: Sending breaking signal");
                // let mut data: [u8; 2] = *b"0!"; // send break
                self.state.replace(Sdi12State::Tx(len, data)); // placeholder length and data
                self.uart_pin.make_output();
                self.uart_pin.clear();

                let interval = self.alarm.ticks_from_ms(MARKIING_INTERVAL_MS);
                self.alarm.set_alarm(self.alarm.now(), interval);
            }
            Sdi12State::Tx(len, data) => {
                kernel::debug!("SDI12: Transmitting data");
                kernel::debug!("SDI12: Data length: {}", len);
                kernel::debug!(
                    "SDI12: Data content: {}",
                    core::str::from_utf8(&data[..len]).unwrap_or("<invalid utf8>")
                );
                match self.uart.transmit_buffer(data, len) {
                    Ok(()) => {
                        kernel::debug!("SDI12: UART transmit started successfully");
                    }
                    Err((err, buf)) => {
                        kernel::debug!("SDI12: UART transmit failed with error: {:?}", err);
                        // Transmission failed, return to Idle state and notify client.
                        self.state.replace(Sdi12State::Idle);
                        self.tx_client
                            .map(|client| client.transmitted_buffer(buf, len, Err(err)));
                    }
                }
            }
        };
    }
}

impl<'a, U: Uart<'a>, A: Alarm<'a>> uart::TransmitClient for Sdi12<'a, U, A> {
    fn transmitted_buffer(
        &self,
        buffer: &'static mut [u8],
        length: usize,
        status: Result<(), ErrorCode>,
    ) {
        // Transmission complete, set command pin high, return to Idle state and notify client.
        self.command_pin.set(); // set control pin high for RX mode
        kernel::debug!("SDI12: Transmission complete");
        self.state.replace(Sdi12State::Idle);
        self.tx_client
            .map(|client| client.transmitted_buffer(buffer, length, status));
    }
}

impl<'a, U: Uart<'a>, A: Alarm<'a>> uart::ReceiveClient for Sdi12<'a, U, A> {
    fn received_buffer(
        &self,
        buffer: &'static mut [u8],
        length: usize,
        status: Result<(), ErrorCode>,
        error: Error,
    ) {
        kernel::debug!("SDI12: Reception complete");
        self.rx_client
            .map(|client| client.receive_buffer(buffer, length, status, error));
    }
}
