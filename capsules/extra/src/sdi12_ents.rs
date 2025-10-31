use capsules_core::driver;
use capsules_core::virtualizers::virtual_alarm::VirtualMuxAlarm;
use capsules_core::virtualizers::virtual_uart::UartDevice;
use core::cell::Cell;
use kernel::debug;
use kernel::errorcode::{into_statuscode, ErrorCode};
use kernel::hil::gpio::Pin;
use kernel::hil::time::{Alarm, AlarmClient, ConvertTicks, Frequency, Ticks, Time, Timer};
use kernel::hil::uart::TransmitClient;
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
pub struct Sdi12Ents<'a, U, A>
where
    U: kernel::hil::uart::Transmit<'a> + kernel::hil::uart::Receive<'a>,
    A: kernel::hil::time::Alarm<'a>,
{
    uart: &'a U,
    state: Cell<State>,
    tx_buffer: TakeCell<'static, [u8]>,
    command_pin: &'a dyn Pin,
    alarm: &'a A,
}

impl<'a, U, A> Sdi12Ents<'a, U, A>
where
    U: kernel::hil::uart::Transmit<'a> + kernel::hil::uart::Receive<'a>,
    A: kernel::hil::time::Alarm<'a>,
{
    pub fn new(
        uart: &'a U,
        tx_buffer: &'static mut [u8],
        command_pin: &'a dyn Pin,
        alarm: &'a A,
    ) -> Sdi12Ents<'a, U, A> {
        debug!("Initializing SDI12 capsule");
        Sdi12Ents {
            uart,
            state: Cell::new(State::Idle),
            tx_buffer: TakeCell::new(tx_buffer),
            command_pin,
            alarm,
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
        debug!("Waking SDI12 sensors");
        self.command_pin.make_output();
        self.command_pin.clear();
        let interval = self.alarm.ticks_from_ms(WAKE_SENSORS_INTERVAL_MS);
        self.alarm.set_alarm(self.alarm.now(), interval);
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
        self.state.set(State::SendingCommand);

        let command_bytes = command.as_bytes();
        let len = size.min(command_bytes.len()); // prevent overflow
        let buffer = self.tx_buffer.take().unwrap();
        buffer[..len].copy_from_slice(&command_bytes[..len]);
        let status_result = self.uart.transmit_buffer(buffer, len);
        debug!(
            "Beginning SDI12 transmit: command={}, status_result={:?}, setting control pin high for RX mode",
            command, status_result
        );
        self.command_pin.set();
        match status_result {
            Ok(()) => Ok(Sdi12Status::Sdi12Ok),
            Err(_) => Err(Sdi12Status::Sdi12Error),
        }
    }

    /**
     ******************************************************************************
     * @brief    Acknowledge Active
     *
     * @return   Sdi12Status
     ******************************************************************************
     */
    pub fn sdi12_ack_active(&self) -> Sdi12Status {
        self.state.set(State::SendingCommand);
        let buffer = "0!!!!!!!!".as_bytes();
        let len = buffer.len();
        let tx_buffer = self.tx_buffer.take().unwrap();
        tx_buffer[..len].copy_from_slice(&buffer[..len]);
        let status_result = self.uart.transmit_buffer(&mut tx_buffer[..len], len);
        debug!("Sending SDI12 Acknowledge Active to Address 0");
        match status_result {
            Ok(()) => Sdi12Status::Sdi12Ok,
            Err(_) => Sdi12Status::Sdi12Error,
        }
    }
}

impl<'a, U, A> SyscallDriver for Sdi12Ents<'a, U, A>
where
    U: kernel::hil::uart::Transmit<'a> + kernel::hil::uart::Receive<'a>,
    A: kernel::hil::time::Alarm<'a>,
{
    fn command(
        &self,
        command_num: usize,
        _: usize,
        _: usize,
        processid: ProcessId,
    ) -> CommandReturn {
        debug!("command syscall executing");
        // /self.sdi12_wake_sensors();
        self.command_pin.clear();
        self.sdi12_send_command("0!", 3);
        CommandReturn::success()
    }

    fn allocate_grant(&self, processid: ProcessId) -> Result<(), kernel::process::Error> {
        // Allocation is performed implicitly when the grant region is entered.
        //self.apps.enter(processid, |_, _| {});
        Ok(())
    }
}

impl<'a, U, A> AlarmClient for Sdi12Ents<'a, U, A>
where
    A: kernel::hil::time::Alarm<'a>,
    U: kernel::hil::uart::Transmit<'a> + kernel::hil::uart::Receive<'a>,
{
    fn alarm(&self) {
        // match self.state.get() {
        //     State::WakingSensors => {
        //         self.command_pin.clear();
        //         self.state.set(State::Idle);
        //     }
        //     _ => {}
        // }
        debug!("SDI12 Alarm fired, sending command");
        self.sdi12_send_command("0!", 3);
        self.state.set(State::Idle);
    }
}

impl<'a, U, A> TransmitClient for Sdi12Ents<'a, U, A>
where
    A: kernel::hil::time::Alarm<'a>,
    U: kernel::hil::uart::Transmit<'a> + kernel::hil::uart::Receive<'a>,
{
    fn transmitted_buffer(
        &self,
        buffer: &'static mut [u8],
        _length: usize,
        _status: Result<(), ErrorCode>,
    ) {
        debug!(
            "SDI12 Transmit complete, returning buffer: STATUS={:?}",
            _status
        );
        self.tx_buffer.replace(buffer);
    }
}
