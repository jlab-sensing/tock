add extern crate alloc;
use alloc::string::String;
use core::cell::Cell;
use kernel::hil::gpio;
use kernel::hil::uart::{self, Client, Uart};
use kernel::utilities::cells::{OptionalCell, TakeCell};
use kernel::hil::time::{Alarm, AlarmClient, ConvertTicks};
use kernel::{ErrorCode, ProcessId};

#[derive(Copy, Clone, PartialEq)]

enum Sdi12Status {
    Sdi12Ok = 0,
    Sdi12Error = -1,
    Sdi12TimeoutOnRead = -2,
    Sdi12ParsingError = -3,
}

struct Sdi12MeasurmentValues {
    address: char,
    time: u16,
    numvalues: u8,
}
pub enum State {
    Idle,
    AddressRequest,
    GivingAddress,
    WakingSensors,
    SendingCommand,
    ReadingResponse,
}
pub struct Sdi12Ents<'a> {
    uart: &'a dyn Uart<'a>,
    state: Cell<State>,
    buffer: TakeCell<'static, [u8]>,
    client: OptionalCell<&'a dyn Client>,
    command_pin: &'a dyn gpio::Output<'a>,
    alarm: &'a A,
}

impl Sdi12Ents<'a> {
    pub fn new(uart: &'a dyn Uart<'a>, buffer: &'static mut [u8]) -> Sdi12Ents<'a> {
        Sdi12Ents {
            uart: uart,
            state: Cell::new(State::Idle),
            buffer: TakeCell::new(buffer),
            client: OptionalCell::empty(),
            command_pin: command_pin,
            alarm: alarm,
        }
    }
    /**
     ******************************************************************************
     * @brief    Wake all sensors on the data line.
     *
     * @param    void
     * @return   void
     ******************************************************************************
     */
    pub fn sdi12_wake_sensors() {
        self.command_pin.clear();
        self.alarm.set_alarm(self.alarm.now(), self.alarm.ticks_from_ms(20));
    }

    /**
    ******************************************************************************
    * @brief    Send a command via SDI12
    *
    * @param    const char *, command
    * @return   SDI12Status
    ******************************************************************************
    */
    pub fn sdi12_send_command(command: &str, size: u8) -> Result<Sdi12Status, Sdi12Status> {
        self.state.set(State::WakingSensors);
        self.sdi12_wake_sensors();
        self.state.set(State::SendingCommand);
        let err: ErrorCode = self.uart.transmit_buffer(command.as_bytes(), size);
        if err == ErrorCode::SUCCESS {
            Ok(Sdi12Status::Sdi12Ok)
        } else {
            Err(Sdi12Status::Sdi12Error)
        }

        self.command_pin.set(); // set command pin to RX mode
    }

    // pub fn sdi12_read_response() {}

    // pub fn sdi12_get_measurement() {}

    // pub fn sdi12_get_address() {}

    // Internal helper function for parsing the response from a sensor after a
    // measurement command has been sent.
    fn parse_measurment_response(
        &self,
        response: &str,
        addr: char,
        measurment_info: &mut Sdi12MeasurmentValues,
    ) -> Result<Sdi12Status, Sdi12Status> {
        measurment_info.address = response.chars().nth(0).unwrap();
        measurment_info.time = response[1..4].parse().unwrap(); // chars 1,2,3
        measurment_info.numvalues = response[4..5].parse().unwrap(); // char 4

        if measurment_info.address == addr {
            Ok(Sdi12Status::Sdi12Ok)
        } else {
            Err(Sdi12Status::Sdi12ParsingError)
        }
    }

    // Internal helper function for parsing a sensor's service request
    fn parse_service_request(&self, request: &str, addr: char) -> Result<Sdi12Status, Sdi12Status> {
        let expectedResponse = String::from("a\r\n");
        let expectedResponse = expectedResponse.replace("a", &addr.to_string());

        if request == expectedResponse {
            Ok(Sdi12Status::Sdi12Ok)
        } else {
            Err(Sdi12Status::Sdi12ParsingError)
        }
    }
}

impl<'a, A: Alarm<'a>> AlarmClient for Sdi12Ents<'a, A> {
    fn alarm(&self) {
        self.command_pin.set(); // release after 20ms
        self.state.set(State::Idle); // or trigger UART work
    }
}
