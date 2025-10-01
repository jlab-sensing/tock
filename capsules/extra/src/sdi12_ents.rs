use capsules_core::virtualizers::virtual_uart::UartDevice;
use core::cell::Cell;
use kernel::errorcode::{into_statuscode, ErrorCode};
use kernel::hil::gpio::Output;
use kernel::hil::time::{ConvertTicks, Frequency, Ticks, Timer};
use kernel::hil::uart::{Client, ReceiveClient, Uart};
use kernel::utilities::cells::{OptionalCell, TakeCell};
use kernel::ProcessId;

extern crate alloc;
use alloc::format;

const REQUEST_MEASURMENT_RESPONSE_SIZE: usize = 7;
const MEASURMENT_RESPONSE_SIZE: usize = 30;
const SERVICE_REQUEST_SIZE: usize = 3;
const WAKE_SENSORS_INTERVAL_MS: u32 = 20;

enum Sdi12Status {
    Sdi12Ok = 0,
    Sdi12Error = -1,
    Sdi12TimeoutOnRead = -2,
    Sdi12ParsingError = -3,
}

#[derive(Copy, Clone, PartialEq)]
struct Sdi12MeasurmentValues {
    address: u8,
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
pub struct Sdi12Ents<'a, T, F>
where
    T: Ticks,
    F: Frequency,
{
    uart: &'a UartDevice<'a>,
    state: Cell<State>,
    // tx_buffer: TakeCell<'static, [u8]>,
    // tx_in_progress: OptionalCell<ProcessId>,
    // rx_buffer: TakeCell<'static, [u8]>,
    // rx_in_progress: OptionalCell<ProcessId>,
    // client: OptionalCell<&'a dyn Client>,
    command_pin: &'a dyn Output,
    timer: &'a dyn Timer<'a, Frequency = F, Ticks = T>,
    ticks: &'a dyn ConvertTicks<T>,
}

impl<'a, T, F> Sdi12Ents<'a, T, F>
where
    T: Ticks,
    F: Frequency,
{
    pub fn new(
        uart: &'a UartDevice<'a>,
        // tx_buffer: &'static mut [u8],
        // rx_buffer: &'static mut [u8],
        command_pin: &'a dyn Output,
        timer: &'a dyn Timer<'a, Frequency = F, Ticks = T>,
        ticks: &'a dyn ConvertTicks<T>,
    ) -> Sdi12Ents<'a, T, F> {
        Sdi12Ents {
            uart: uart,
            state: Cell::new(State::Idle),
            // tx_buffer: TakeCell::new(tx_buffer),
            // tx_in_progress: OptionalCell::empty(),
            // rx_buffer: TakeCell::new(rx_buffer),
            // rx_in_progress: OptionalCell::empty(),
            // client: OptionalCell::empty(),
            command_pin: command_pin,
            timer: timer,
            ticks: ticks,
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
    pub fn sdi12_wake_sensors(&self) {
        self.command_pin.clear();
        let interval: T = self.ticks.ticks_from_ms(WAKE_SENSORS_INTERVAL_MS);
        self.timer.oneshot(interval); // hold line low for 20ms
    }

    /**
     ******************************************************************************
     * @brief    Send a command via SDI12
     *
     * @param    str, command
     * @return   Sdi12Status
     ******************************************************************************
     */
    pub fn sdi12_send_command(
        &self,
        command: &str,
        size: usize,
    ) -> Result<Sdi12Status, Sdi12Status> {
        self.state.set(State::WakingSensors);
        self.sdi12_wake_sensors();
        self.state.set(State::SendingCommand);

        if let Some(buffer) = self.tx_buffer.take() {
            let command_bytes = command.as_bytes();
            let len = size.min(buffer.len()); // prevent overflow
            buffer[..len].copy_from_slice(&command_bytes[..len]);

            let status_result = self.uart.transmit_buffer(buffer, len); // Move into the UART transmit buffer
            let status_code = match status_result {
                Ok(()) => 0,
                Err((_err, tx_buffer)) => {
                    self.uart.tx_buffer.replace(tx_buffer);
                    self.command_pin.set();
                    1
                }
            };

            if status_code == 0 {
                Ok(Sdi12Status::Sdi12Ok)
            } else {
                Err(Sdi12Status::Sdi12Error)
            }
        } else {
            Err(Sdi12Status::Sdi12Error) // tx_buffer unavailable
        }
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
    pub fn sdi12_read_data(&self, buffer_size: usize) -> Result<Sdi12Status, Sdi12Status> {
        self.state.set(State::ReadingResponse);
        self.command_pin.set(); // set command pin to RX mode

        if let Some(buffer) = self.rx_buffer.take() {
            let len = buffer_size.min(buffer.len()); // prevent overflow

            let status_result = self.uart.receive_buffer(buffer, len); // Move into the UART receive buffer
            let status_code = match status_result {
                Ok(()) => 0,
                Err((_err, rx_buffer)) => {
                    self.rx_buffer.replace(rx_buffer);
                    1
                }
            };

            if status_code == 0 {
                Ok(Sdi12Status::Sdi12Ok)
            } else {
                Err(Sdi12Status::Sdi12Error)
            }
        } else {
            Err(Sdi12Status::Sdi12Error) // rx_buffer unavailable
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
    pub fn sdi12_get_measurement(
        &self,
        addr: u8,
        measurment_info: &mut Sdi12MeasurmentValues,
        measurment_data: &mut char,
        timeout_millies: u16,
    ) -> Result<Sdi12Status, Sdi12Status> {
        let request_measurment_command = format!("{}M!\r\n", addr as char); // Command to request measurement ("0!\r\n" for example)
        let send_data_command = format!("{}D0!\r\n", addr as char); // Command to get data ("0D0\r\n" for example)
        let mut measurment_response: [u8; REQUEST_MEASURMENT_RESPONSE_SIZE] =
            [0; REQUEST_MEASURMENT_RESPONSE_SIZE]; // Buffer to store the response from the sensor for a request measurment command
        let mut measurment_data: [u8; MEASURMENT_RESPONSE_SIZE] = [0; MEASURMENT_RESPONSE_SIZE]; // Buffer to store the response from the sensor for a measurment data command

        // Send the measurment request command and wait for immediate response
        let size = request_measurment_command.len() as u8;
        let status = self.sdi12_send_command(&request_measurment_command, size)?;

        status = self.sdi12_read_data(REQUEST_MEASURMENT_RESPONSE_SIZE)?;

        // Check if the addresses match from the response above.
        // The response from a teros is the same every
        // time so we're going to leave it for now
        status = self.parse_measurment_response(&mut measurment_response, addr, measurment_info)?;

        self.sdi12_read_data(SERVICE_REQUEST_SIZE)?; // Read the service request
        status = self.parse_service_request(&mut measurment_data, addr)?; // Parse the service request

        status = self.sdi12_send_command(&send_data_command, send_data_command.len() as usize)?; // Send the command to get the data
        status = self.sdi12_read_data(MEASURMENT_RESPONSE_SIZE)?;
        // Read the measurment data
    }

    /**
     * @brief Get the address of the current SDI-12 sensor. Only a single sensor can be connected to the bus at a time.
     *
     * @param addr Mutable address
     * @return Sdi12Status
     */
    pub fn sdi12_get_address(
        &self,
        addr: &mut u8,
        timeout_millies: u16,
    ) -> Result<Sdi12Status, Sdi12Status> {
        let get_address_command = "?!\r\n"; // Command to get the address
        let response: [u8; 4] = [0; 4]; // Buffer to store the response

        self.sdi12_send_command(get_address_command, get_address_command.len() as u8)?;
        let status = self.sdi12_read_data(&mut response, 4)?;
    }

    // Internal helper function for parsing the response from a sensor after a
    // measurement command has been sent.
    fn parse_measurment_response(
        &self,
        response: &mut [u8],
        addr: u8,
        measurment_info: &mut Sdi12MeasurmentValues,
    ) -> Result<Sdi12Status, Sdi12Status> {
        measurment_info.address = response[0]; // First byte is the address
        let d1 = response[1].wrapping_sub(b'0') as u16;
        let d2 = response[2].wrapping_sub(b'0') as u16;
        let d3 = response[3].wrapping_sub(b'0') as u16;
        measurment_info.time = d1 * 100 + d2 * 10 + d3; // Convert ASCII to integer
        measurment_info.numvalues = response[4]; // Fifth byte is the number of values

        if measurment_info.address == addr {
            Ok(Sdi12Status::Sdi12Ok)
        } else {
            Err(Sdi12Status::Sdi12ParsingError)
        }
    }

    // Internal helper function for parsing a sensor's service request
    fn parse_service_request(
        &self,
        request: &mut [u8],
        addr: u8,
    ) -> Result<Sdi12Status, Sdi12Status> {
        if request[0] == addr && request[1] == b'!' && request[2] == b'\r' && request[3] == b'\n' {
            Ok(Sdi12Status::Sdi12Ok)
        } else {
            Err(Sdi12Status::Sdi12ParsingError)
        }
    }
}

impl<T: Ticks, F: Frequency> ReceiveClient for Sdi12Ents<'_, T, F> {
    fn received_buffer(
        &self,
        rx_buffer: &'static mut [u8],
        rx_len: usize,
        rval: Result<(), ErrorCode>,
        error: Error,
    ) {
        self.state.set(State::Idle);

        self.client.map(|client| {
            // Notify whoever registered as the SDI-12 client
            self.RecieveClient.recieved_buffer(buffer, rx_len, rcode);
        });
        self.rx_buffer.replace(rx_buffer); // Return capsule
    }
}
