use kernel::hil::gpio::{Configure, Output};
use kernel::hil::sdi12;
use kernel::hil::time::{Alarm, AlarmClient, ConvertTicks};
use kernel::hil::uart::{self, Uart};

use kernel::utilities::cells::{MapCell, OptionalCell};

use kernel::ErrorCode;

const WAKE_SENSORS_INTERVAL_MS: u32 = 13;
const MARKIING_INTERVAL_MS: u32 = 9;

use crate::gpio::{AlternateFunction, Mode};

enum Sdi12State {
    TxBreak(),
    TxMarking(),
    Tx(usize, &'static mut [u8]),
    Idle,
}

pub struct Sdi12<'a, U: Uart<'a>, A: Alarm<'a>> {
    uart: &'a U,
    uart_pin: &'a crate::gpio::Pin<'a>,
    alarm: &'a A,
    state: MapCell<Sdi12State>,
    client: OptionalCell<&'a dyn sdi12::TransmitClient>,
}

impl<'a, U: Uart<'a>, A: Alarm<'a>> Sdi12<'a, U, A> {
    pub fn new(uart: &'a U, uart_pin: &'a crate::gpio::Pin<'a>, alarm: &'a A) -> Self {
        Sdi12 {
            uart,
            uart_pin,
            alarm,
            state: MapCell::new(Sdi12State::Idle),
            client: OptionalCell::empty(),
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

        match state {
            Sdi12State::Idle => {
                self.state.replace(Sdi12State::TxBreak());
            }
            Sdi12State::Tx(enum_len, enum_data) => {
                // Already transmitting.
                self.state.replace(Sdi12State::Tx(enum_len, enum_data));
                return Err((ErrorCode::BUSY, data));
            }
            Sdi12State::TxBreak() | Sdi12State::TxMarking() => {
                // In a pre-transmitting state
                self.state.replace(state);
                return Err((ErrorCode::BUSY, data));
            }
        }

        // Hold USART pin high to enable transmission
        self.uart_pin.make_output();
        self.uart_pin.set();

        let interval = self.alarm.ticks_from_ms(WAKE_SENSORS_INTERVAL_MS);
        self.alarm.set_alarm(self.alarm.now(), interval);

        Ok(())
    }

    fn set_transmit_client(&self, client: &'a dyn sdi12::TransmitClient) {
        self.client.set(client);
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
            Sdi12State::TxBreak() => {
                // Time to send marking interval.
                self.state.replace(Sdi12State::TxMarking());
                let interval = self.alarm.ticks_from_ms(MARKIING_INTERVAL_MS);
                self.alarm.set_alarm(self.alarm.now(), interval);
            }
            Sdi12State::TxMarking() => {
                // Time to send data.
                // We unwrap here because this is undefined if the state
                // is None (e.g. should only ever be mapped).
                let state = self.state.take().unwrap();
                match state {
                    Sdi12State::Tx(len, data) => {
                        self.state.replace(Sdi12State::Tx(len, data));
                    }
                    _ => unreachable!("SDI12 Alarm fired in unexpected state."),
                }
            }
            Sdi12State::Tx(len, data) => {
                self.uart
                    .transmit_buffer(data, len)
                    .map_err(|(err, buf)| {
                        // Transmission failed, return to Idle state and notify client.
                        self.state.replace(Sdi12State::Idle);
                        self.client
                            .map(|client| client.transmitted_buffer(buf, len, Err(err)));
                    })
                    .ok();
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
        // Transmission complete, return to Idle state and notify client.
        self.state.replace(Sdi12State::Idle);
        self.client
            .map(|client| client.transmitted_buffer(buffer, length, status));
    }
}
