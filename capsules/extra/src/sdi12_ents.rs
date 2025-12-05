// Licensed under the Apache License, Version 2.0 or the MIT License.
// SPDX-License-Identifier: Apache-2.0 OR MIT
// Copyright Tock Contributors 2022.
// Written by Stephen Taylor, UCSD, 2025

//! Provides userspace with access to sdi12 enviormental sensors. Uses a command pin specific to ENTS hardware
//!
//! Userspace Interface
//! -------------------
//!
//! ### `command` System Call
//!
//! The `command` system call support one argument `cmd` which is used to specify the specific
//! operation, currently the following cmd's are supported:
//!
//! * `0`: check whether the driver exists
//! * `1`: query the address of connected SDI12 sensor, note this requires the command pin to be connected
//! and that if multiple sensors are connected they will all respond to this command. Potentially resulting in bus contention
//!
//!

use capsules_core::driver;
use capsules_core::virtualizers::virtual_alarm::VirtualMuxAlarm;
use capsules_core::virtualizers::virtual_uart::UartDevice;
use core::cell::Cell;
use kernel::debug;
use kernel::errorcode::{into_statuscode, ErrorCode};
use kernel::hil::gpio::Pin;
use kernel::hil::sdi12;
use kernel::hil::sdi12::TransmitClient;
use kernel::hil::time::{Alarm, AlarmClient, ConvertTicks, Frequency, Ticks, Time, Timer};
use kernel::syscall::{CommandReturn, SyscallDriver};
use kernel::utilities::cells::TakeCell;
use kernel::ProcessId;

const REQUEST_MEASURMENT_RESPONSE_SIZE: usize = 7;
const MEASURMENT_RESPONSE_SIZE: usize = 30;
const SERVICE_REQUEST_SIZE: usize = 3;
const WAKE_SENSORS_INTERVAL_MS: u32 = 9;

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
pub struct Sdi12Ents<'a, S: sdi12::Transmit<'a>> {
    state: Cell<State>,
    tx_buffer: TakeCell<'static, [u8]>,
    sdi12: &'a S,
}

impl<'a, S: sdi12::Transmit<'a>> Sdi12Ents<'a, S> {
    pub fn new(tx_buffer: &'static mut [u8], sdi12: &'a S) -> Sdi12Ents<'a, S> {
        debug!("Initializing SDI12 capsule");
        Sdi12Ents {
            state: Cell::new(State::Idle),
            tx_buffer: TakeCell::new(tx_buffer),
            sdi12,
        }
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
        command: usize,
        size: usize,
    ) -> Result<Sdi12Status, Sdi12Status> {
        self.state.set(State::SendingCommand);
        let mut command_bytes: [u8; 4] = [0; 4];
        for i in 0..size {
            // Convert usize to u8 and store in buffer
            command_bytes[i] = (command >> (8 * (size - 1 - i))) as u8;
        }

        // let command_bytes = command.as_bytes();
        // let len = size.min(command.len()); // prevent overflow
        let buffer = self.tx_buffer.take().unwrap();
        buffer[..size].copy_from_slice(&command_bytes[..size]);
        debug!("Beginning capsule call of SDI12 driver");
        let status_result = self.sdi12.transmit(buffer, size);
        match status_result {
            Ok(()) => Ok(Sdi12Status::Sdi12Ok),
            Err(_) => Err(Sdi12Status::Sdi12Error),
        }
    }
}

impl<'a, S> SyscallDriver for Sdi12Ents<'a, S>
where
    S: sdi12::Transmit<'a>,
{
    fn command(
        &self,
        _command_num: usize,
        data1: usize,
        data2: usize,
        _processid: ProcessId,
    ) -> CommandReturn {
        // debug!("command syscall executing");
        kernel::debug!("SDI12 capsule command syscall {}", _command_num);
        kernel::debug!("data1: {}, data2: {}", data1, data2);
        match _command_num {
            // Driver existence check
            0 => CommandReturn::success(),
            // test take measurment command
            1 => match self.sdi12_send_command(data1, data2) {
                Ok(_) => CommandReturn::success(),
                _ => CommandReturn::failure(ErrorCode::FAIL),
            },
            2 => {
                // send address command, may create bus contention if multiple sensors are connected
                // let command_str = "?!";
                // let size = 2;
                match self.sdi12_send_command(data1, data2) {
                    Ok(_) => CommandReturn::success(),
                    _ => CommandReturn::failure(ErrorCode::FAIL),
                }
            }
            _ => CommandReturn::failure(ErrorCode::INVAL),
        }
    }

    fn allow_userspace_readable(
        &self,
        app: ProcessId,
        which: usize,
        slice: kernel::processbuffer::UserspaceReadableProcessBuffer,
    ) -> Result<
        kernel::processbuffer::UserspaceReadableProcessBuffer,
        (
            kernel::processbuffer::UserspaceReadableProcessBuffer,
            ErrorCode,
        ),
    > {
        match which {
            0 => Ok(slice),
            _ => Err((slice, ErrorCode::INVAL)),
        }
    }

    fn allocate_grant(&self, processid: ProcessId) -> Result<(), kernel::process::Error> {
        // Allocation is performed implicitly when the grant region is entered.
        self.apps.enter(processid, |_, _| {})
    }
}

impl<'a, S: sdi12::Transmit<'a>> TransmitClient for Sdi12Ents<'a, S> {
    fn transmitted_buffer(
        &self,
        buffer: &'static mut [u8],
        _length: usize,
        _status: Result<(), ErrorCode>,
    ) {
        debug!("SDI12 capsule Transmit complete, buffer returned");
        // Put the buffer back into the TakeCell for reuse
        self.tx_buffer.replace(buffer);
        self.state.set(State::Idle);
    }
}
