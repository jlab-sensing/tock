use capsules_core::driver;
use capsules_core::virtualizers::virtual_uart::UartDevice;
use core::cell::Cell;
use kernel::debug;
use kernel::errorcode::{into_statuscode, ErrorCode};
use kernel::hil::gpio::Output;
use kernel::hil::time::{ConvertTicks, Frequency, Ticks, Timer};
use kernel::hil::uart::{Client, ReceiveClient, Uart};
use kernel::syscall::{CommandReturn, SyscallDriver};
use kernel::utilities::cells::{OptionalCell, TakeCell};
use kernel::ProcessId;

const REQUEST_MEASURMENT_RESPONSE_SIZE: usize = 7;
const MEASURMENT_RESPONSE_SIZE: usize = 30;
const SERVICE_REQUEST_SIZE: usize = 3;
const WAKE_SENSORS_INTERVAL_MS: u32 = 20;

pub const DRIVER_NUM: usize = driver::NUM::Sdi12Ents as usize;

pub enum Sdi12Status {
    Sdi12Ok = 0,
    Sdi12Error = -1,
    Sdi12TimeoutOnRead = -2,
    Sdi12ParsingError = -3,
}

#[derive(Copy, Clone, PartialEq)]
pub struct Sdi12MeasurmentValues {
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
pub struct Sdi12Ents<'a, U>
where
    U: kernel::hil::uart::Transmit<'a> + kernel::hil::uart::Receive<'a>,
{
    uart: &'a U,
    state: Cell<State>,
    tx_buffer: TakeCell<'static, [u8]>,
}

impl<'a, U> Sdi12Ents<'a, U>
where
    U: kernel::hil::uart::Transmit<'a> + kernel::hil::uart::Receive<'a>,
{
    pub fn new(uart: &'a U, tx_buffer: &'static mut [u8]) -> Sdi12Ents<'a, U> {
        debug!("Initializing SDI12 capsule");
        Sdi12Ents {
            uart,
            state: Cell::new(State::Idle),
            tx_buffer: TakeCell::new(tx_buffer),
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
    // pub fn sdi12_wake_sensors(&self) {
    //     self.command_pin.clear();
    //     let interval: T = self.ticks.ticks_from_ms(WAKE_SENSORS_INTERVAL_MS);
    //     self.timer.oneshot(interval); // hold line low for 20ms
    // }

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
        self.state.set(State::SendingCommand);

        let command_bytes = command.as_bytes();
        let len = size.min(command_bytes.len()); // prevent overflow
        let buffer = self.tx_buffer.take().unwrap();
        buffer[..len].copy_from_slice(&command_bytes[..len]);
        let status_result = self.uart.transmit_buffer(buffer, len);
        debug!("Beginning SDI12 transmit: command={}", command);
        match status_result {
            Ok(()) => Ok(Sdi12Status::Sdi12Ok),
            Err(_) => Err(Sdi12Status::Sdi12Error),
        }
    }
}

impl<'a, U> SyscallDriver for Sdi12Ents<'a, U>
where
    U: kernel::hil::uart::Transmit<'a> + kernel::hil::uart::Receive<'a>,
{
    fn command(
        &self,
        command_num: usize,
        _: usize,
        _: usize,
        processid: ProcessId,
    ) -> CommandReturn {
        panic!("syscall has been successfully routed to sdi12_ents");
        self.sdi12_send_command("0I!", 3);
        CommandReturn::success()
    }

    fn allocate_grant(&self, processid: ProcessId) -> Result<(), kernel::process::Error> {
        // Allocation is performed implicitly when the grant region is entered.
        // self.apps.enter(processid, |_, _| {})
        Ok(())
    }
}
