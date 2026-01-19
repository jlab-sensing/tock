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
use core::cell::Cell;
use kernel::debug;
use kernel::errorcode::ErrorCode;
use kernel::grant::UpcallCount;
use kernel::grant::{AllowRoCount, AllowRwCount, Grant};
use kernel::hil::sdi12;
use kernel::hil::sdi12::TransmitClient;
use kernel::processbuffer::ReadableProcessBuffer;
use kernel::processbuffer::WriteableProcessBuffer;
use kernel::syscall::{CommandReturn, SyscallDriver};
use kernel::utilities::cells::OptionalCell;
use kernel::utilities::cells::TakeCell;
use kernel::ProcessId;

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

/// IDs for subscribed upcalls.
mod upcall {
    /// Sdi12 received.
    pub const SDI12_RX: usize = 0;
    /// Sdi12 transmit complete.
    pub const SDI12_TX: usize = 1;
    /// Number of upcalls.
    pub const COUNT: u8 = 2;
}

mod ro_allow {
    /// Tx buffer for SDI12 transmit.
    pub const TX_BUFFER: usize = 0;
    /// Number of read-only allow buffers.
    pub const COUNT: u8 = 1;
}

mod rw_allow {
    /// Rx buffer for SDI12 receive.
    pub const RX_BUFFER: usize = 0;
    /// Number of read-write allow buffers.
    pub const COUNT: u8 = 1;
}

pub struct Sdi12Ents<'a, S: sdi12::Transmit<'a> + sdi12::Receive<'a>> {
    state: Cell<State>,
    tx_buffer: TakeCell<'static, [u8]>,
    rx_buffer: TakeCell<'static, [u8]>,
    sdi12: &'a S,
    grant: Grant<
        App,
        UpcallCount<{ upcall::COUNT }>,
        AllowRoCount<{ ro_allow::COUNT }>,
        AllowRwCount<{ rw_allow::COUNT }>,
    >,
    tx_in_progress: OptionalCell<ProcessId>,
    rx_in_progress: OptionalCell<ProcessId>,
}

/// Holds buffers and whatnot that the application has passed us.
#[derive(Default)]
pub struct App;

impl<'a, S: sdi12::Transmit<'a> + sdi12::Receive<'a>> Sdi12Ents<'a, S> {
    pub fn new(
        tx_buffer: &'static mut [u8],
        rx_buffer: &'static mut [u8],
        sdi12: &'a S,
        grant: Grant<
            App,
            UpcallCount<{ upcall::COUNT }>,
            AllowRoCount<{ ro_allow::COUNT }>,
            AllowRwCount<{ rw_allow::COUNT }>,
        >,
    ) -> Sdi12Ents<'a, S> {
        debug!("Initializing SDI12 capsule");
        Sdi12Ents {
            state: Cell::new(State::Idle),
            tx_buffer: TakeCell::new(tx_buffer),
            rx_buffer: TakeCell::new(rx_buffer),
            sdi12,
            grant,
            tx_in_progress: OptionalCell::empty(),
            rx_in_progress: OptionalCell::empty(),
        }
    }

    /**
     ******************************************************************************
     * @brief    Send a command via SDI12
     *
     * @param    self,
     * @param    str, command
     * @return   Sdi12Status
     ******************************************************************************
     */
    // pub fn sdi12_send_command(
    //     &self,
    //     processid: ProcessId,
    //     command: usize,
    //     size: usize,
    // ) -> Result<Sdi12Status, Sdi12Status> {
    //     self.state.set(State::SendingCommand);
    //     self.tx_in_progress.set(processid);
    //     let mut command_bytes: [u8; 4] = [0; 4];
    //     for i in 0..size {
    //         // Convert usize to u8 and store in buffer
    //         command_bytes[i] = (command >> (8 * (size - 1 - i))) as u8;
    //     }

    //     // let command_bytes = command.as_bytes();
    //     // let len = size.min(command.len()); // prevent overflow
    //     let buffer = self.tx_buffer.take().unwrap();
    //     buffer[..size].copy_from_slice(&command_bytes[..size]);
    //     debug!("Beginning capsule call of SDI12 driver");
    //     let status_result = self.sdi12.transmit(buffer, size);
    //     match status_result {
    //         Ok(()) => Ok(Sdi12Status::Sdi12Ok),
    //         Err(_) => {
    //             self.tx_in_progress.clear();  // clear if error
    //             Err(Sdi12Status::Sdi12Error)
    //         }
    //     }
    // }

    /**
     ******************************************************************************
     * @brief    Send data previously copied into tx_buffer
     *
     * @param    self,
     * @param    len, usize
     * @return   Result<(), ErrorCode>
     ******************************************************************************
     */
    pub fn sdi12_send_from_buffer(&self, len: usize) -> Result<(), ErrorCode> {
        self.state.set(State::SendingCommand);

        if let Some(buffer) = self.tx_buffer.take() {
            debug!("Beginning capsule call of SDI12 driver");
            debug!("Data to send: {:?}", &buffer[..len]);
            match self.sdi12.transmit(buffer, len) {
                Ok(()) => Ok(()),
                Err((e, buf)) => {
                    self.tx_buffer.replace(buf);
                    Err(e)
                }
            }
        } else {
            Err(ErrorCode::NOMEM)
        }
    }

    /**
     ******************************************************************************
     * @brief    Start a receive via SDI12
     *
     * @param    self,
     * @param    processid, ProcessID
     * @param    size, usize
     * @return   Sdi12Status
     ******************************************************************************
     */
    pub fn sdi12_start_receive(&self, size: usize) -> Result<Sdi12Status, Sdi12Status> {
        // take a kernel rx buffer
        if let Some(buf) = self.rx_buffer.take() {
            match self.sdi12.receive(buf, size) {
                Ok(()) => {
                    self.state.set(State::ReadingResponse);
                    Ok(Sdi12Status::Sdi12Ok)
                }
                Err((_ecode, returned_buf)) => {
                    // restore kernel buffer and return a generic SDI12 error
                    self.rx_buffer.replace(returned_buf);
                    Err(Sdi12Status::Sdi12Error)
                }
            }
        } else {
            Err(Sdi12Status::Sdi12Error)
        }
    }
}

impl<'a, S> SyscallDriver for Sdi12Ents<'a, S>
where
    S: sdi12::Transmit<'a> + sdi12::Receive<'a>,
{
    fn command(
        &self,
        _command_num: usize,
        data1: usize,
        data2: usize,
        processid: ProcessId,
    ) -> CommandReturn {
        // debug!("command syscall executing");
        kernel::debug!("SDI12 capsule command syscall {}", _command_num);
        kernel::debug!("data1: {}, data2: {}", data1, data2);
        match _command_num {
            // Driver existence check
            0 => CommandReturn::success(),
            // test send data command
            1 => {
                self.tx_in_progress.set(processid);
                // copy data from userspace buffer to kernel
                let len = self.grant.enter(processid, |_app, kernel_data| {
                    let mut copied_len = 0;
                    if let Ok(buffer) = kernel_data.get_readonly_processbuffer(ro_allow::TX_BUFFER)
                    {
                        let _ = buffer.enter(|data| {
                            let max_len = data.len().min(self.tx_buffer.map_or(0, |b| b.len()));
                            self.tx_buffer.map(|tx_buf| {
                                for i in 0..max_len {
                                    tx_buf[i] = data[i].get();
                                }
                                copied_len = max_len;
                            });
                        });
                    }
                    copied_len
                });

                match len {
                    Ok(l) if l > 0 => match self.sdi12_send_from_buffer(l) {
                        Ok(()) => CommandReturn::success(),
                        Err(_) => {
                            self.tx_in_progress.clear();
                            CommandReturn::failure(ErrorCode::FAIL)
                        }
                    },
                    _ => {
                        self.tx_in_progress.clear();
                        CommandReturn::failure(ErrorCode::INVAL)
                    }
                }
            }
            2 => {
                self.rx_in_progress.set(processid);
                // test read data command
                let size = data2;

                // start receive and record process
                match self.sdi12_start_receive(size) {
                    Ok(_) => CommandReturn::success(),
                    Err(_) => CommandReturn::failure(ErrorCode::FAIL),
                }
            }
            3 => CommandReturn::success(),
            _ => CommandReturn::failure(ErrorCode::INVAL),
        }
    }

    fn allocate_grant(&self, processid: ProcessId) -> Result<(), kernel::process::Error> {
        // Allocation is performed implicitly when the grant region is entered.
        kernel::debug!("Allocating SDI12 capsule grant for process {:?}", processid);
        self.grant.enter(processid, |_, _| {})
    }
}

impl<'a, S: sdi12::Transmit<'a> + sdi12::Receive<'a>> TransmitClient for Sdi12Ents<'a, S> {
    fn transmitted_buffer(
        &self,
        buffer: &'static mut [u8],
        length: usize,
        status: Result<(), ErrorCode>,
    ) {
        debug!("SDI12 capsule Transmit complete, buffer returned");
        // Put the buffer back into the TakeCell for reuse
        self.tx_buffer.replace(buffer);
        self.state.set(State::Idle);

        // Schedule upcall to notify userspace
        self.tx_in_progress.take().map(|processid| {
            let _ = self.grant.enter(processid, |_app, kernel_data| {
                let ret_code = match status {
                    Ok(()) => 0,
                    Err(e) => usize::from(e),
                };
                let _ = kernel_data.schedule_upcall(upcall::SDI12_TX, (ret_code, length, 0));
            });
        });
    }
}

impl<'a, S: sdi12::Transmit<'a> + sdi12::Receive<'a>> sdi12::ReceiveClient for Sdi12Ents<'a, S> {
    fn receive_buffer(
        &self,
        buffer: &'static mut [u8],
        length: usize,
        status: Result<(), ErrorCode>,
        _error: kernel::hil::uart::Error,
    ) {
        debug!("SDI12 capsule Receive complete, {} bytes received", length);

        debug!("SDI12 RAW buffer: {:?}", &buffer[..length]);

        // Copy received data to userspace buffer
        self.rx_in_progress.map(|processid| {
            let _ = self.grant.enter(processid, |_app, kernel_data| {
                if let Ok(user_buf) = kernel_data.get_readwrite_processbuffer(rw_allow::RX_BUFFER) {
                    let _ = user_buf.mut_enter(|dest| {
                        let copy_len = length.min(dest.len());
                        for i in 0..copy_len {
                            dest[i].set(buffer[i] & 0x7F); // Mask off bit 7
                        }
                        debug!("Copied {} bytes to userspace (bit 7 masked)", copy_len);
                    });
                }
            });
        });

        // Put the buffer back into the TakeCell for reuse
        self.rx_buffer.replace(buffer);

        // Schedule upcall to notify userspace
        self.rx_in_progress.take().map(|processid| {
            let _ = self.grant.enter(processid, |_app, kernel_data| {
                let ret_code = match status {
                    Ok(()) => 0,
                    Err(e) => usize::from(e),
                };
                let _ = kernel_data.schedule_upcall(upcall::SDI12_RX, (ret_code, length, 0));
            });
        });
    }
}
