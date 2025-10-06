// Licensed under the Apache License, Version 2.0 or the MIT License.
// SPDX-License-Identifier: Apache-2.0 OR MIT
// Copyright Tock Contributors 2022.

//! A dummy I2C client

use core::cell::Cell;
use core::ptr::addr_of_mut;
use kernel::debug;
use kernel::hil;
use kernel::hil::i2c::{Error, I2CMaster};

// ===========================================
// Scan for I2C Slaves
// ===========================================

struct ScanClient {
    dev_id: Cell<u8>,
    i2c_master: &'static dyn I2CMaster<'static>,
}

impl ScanClient {
    pub fn new(i2c_master: &'static dyn I2CMaster<'static>) -> Self {
        Self {
            dev_id: Cell::new(1),
            i2c_master,
        }
    }
}

impl hil::i2c::I2CHwMasterClient for ScanClient {
    fn command_complete(&self, buffer: &'static mut [u8], status: Result<(), Error>) {
        debug!("I2C command complete");
        let mut dev_id = self.dev_id.get();

        if status == Ok(()) {
            debug!("{:#x}", dev_id);
        }

        /*
        let dev: &dyn I2CMaster<'static> = self.i2c_master;
        if dev_id < 0x7F {
            dev_id += 1;
            self.dev_id.set(dev_id);
            //dev.write(dev_id, buffer, 2).unwrap();
            match dev.write(dev_id, buffer, 2) {
                Ok(()) => {
                    debug!("I2C Device at {:#x}", dev_id);
                }
                Err((e, _)) => {
                    debug!("I2C write error at {:#x}: {:?}", dev_id, e);
                }
            }
        } else {
            debug!(
                "Done scanning for I2C devices. Buffer len: {}",
                buffer.len()
            );
        }
        */
    }
}

/// This test should be called with I2C2, specifically
pub fn i2c_scan_slaves(i2c_master: &'static dyn I2CMaster<'static>) {
    // reset command
    static mut DATA: [u8; 1] = [0b000_0110];

    let dev = i2c_master;

    let i2c_client = unsafe { kernel::static_init!(ScanClient, ScanClient::new(dev)) };
    dev.set_master_client(i2c_client);

    dev.enable();

    debug!("Scanning for I2C devices...");
    dev.write(0x40 << 1, unsafe { &mut *addr_of_mut!(DATA) }, 1).unwrap();
    /*
    dev.write(
        i2c_client.dev_id.get(),
        unsafe { &mut *addr_of_mut!(DATA) },
        2,
    )
    .unwrap();
    */
}
