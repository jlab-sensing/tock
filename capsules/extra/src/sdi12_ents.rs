use core::cell::Cell;
use kernel::hil::gpio;
use kernel::hil::uart::{self, Client, Uart};
use kernel::utilities::cells::{OptionalCell, TakeCell};
use kernel::hil::time::{Timer};
use kernel::{ErrorCode, ProcessId};


const REQUEST_MEASURMENT_RESPONSE_SIZE: usize = 7;
const MEASURMENT_RESPONSE_SIZE: usize = 30;
const SERVICE_REQUEST_SIZE: usize = 3;

enum Sdi12Status {
    Sdi12Ok = 0,
    Sdi12Error = -1,
    Sdi12TimeoutOnRead = -2,
    Sdi12ParsingError = -3,
}

#[derive(Copy, Clone, PartialEq)]
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
    command_pin: &'a dyn gpio::Output,
    timer: &'a dyn Timer<'a, Frequency = u32, Ticks = u32>,
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
    * @param    str, command
    * @return   Sdi12Status
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

        self.command_pin.set(); // set command pin to TX mode
    }

    /**
    ******************************************************************************
    * @brief    Read data from a sensor via SDI-12
    *
    * @param    str, response
    * @param    u16, buffer_size
    * @param    u16, timeout_millis
    * @return   Sdi12Status
    ******************************************************************************
    */
    pub fn sdi12_read_data(response: &str, buffer_size: u16, timeout_millies: u16) -> Result<Sdi12Status, Sdi12Status> {
        self.state.set(State::ReadingResponse);
        self.command_pin.clear(); // set command pin to RX mode
        let err: ErrorCode = self.uart.receive_buffer(self.buffer.take().unwrap(), buffer_size as usize); // read from uart
        if err != ErrorCode::SUCCESS {
            return Err(Sdi12Status::Sdi12Error);
        } else {
            Ok(Sdi12Status::Sdi12Ok)
        }
    }

    /**
    ******************************************************************************
    * @brief    This is a function to read a measurment from a particular sensor.
    *
    * @param    u8 addr, the device address
    * @param    Sdi12MeasurmentValues, a custom struct to store the measurment
    *information returned from start measurment
    * @param    &mut char the measurment data returned
    * @param    u16 timeoutMillis time out in milliseconds
    * @return   Sdi12Status
    ******************************************************************************
    */
    pub fn sdi12_get_measurement(addr: u8, measurment_info: &mut Sdi12MeasurmentValues, measurment_data: &mut char, timeout_millies: u16) -> Result<Sdi12Status, Sdi12Status> {
        let request_measurment_command = format!("{}M!\r\n", addr as char); // Command to request measurement ("0!\r\n" for example)
        let send_data_command = format!("{}D0!\r\n", addr as char); // Command to get data ("0D0\r\n" for example)

        // Send the measurment request command and wait for immediate response
        let size = request_measurment_command.len() as u8;
        let status = self.sdi12_send_command(&request_measurment_command, size)?;

        status = self.sdi12_read_data(measurment_response, REQUEST_MEASURMENT_RESPONSE_SIZE, timeout_millies)?;

        // Check if the addresses match from the response above.
        // The response from a teros is the same every
        // time so we're going to leave it for now
        status = self.parse_measurment_response(measurment_response, addr as char, measurment_info)?; 

        self.sdi12_read_data(measurment_data, SERVICE_REQUEST_SIZE, timeout_millies)?; // Read the service request
        status = self.parse_service_request(measurment_data, addr as char)?; // Parse the service request

        status = self.sdi12_send_command(&send_data_command, send_data_command.len() as u8)?; // Send the command to get the data
        status = self.sdi12_read_data(measurment_data, MEASURMENT_RESPONSE_SIZE, timeout_millies)?; // Read the measurment data
        return status;
    }

    /**
     * @brief Get the address of the current SDI-12 sensor. Only a single sensor can be connected to the bus at a time.
     *
     * @param addr Mutable address
     * @return Sdi12Status
     */
    pub fn sdi12_get_address(addr: &mut u8, timeout_millies: u16) -> Result<Sdi12Status, Sdi12Status> {
        let get_address_command = "?!\r\n"; // Command to get the address
        let mut response: [u8; 4] = [0; 4]; // Buffer to store the response

        let status = self.sdi12_send_command(get_address_command, get_address_command.len() as u8)?;
        status = self.sdi12_read_data(addr, 4, timeout_millies)?;
    }

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
        let expected_response = format!("{}!\r\n", addr);

        if request == expected_response {
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
