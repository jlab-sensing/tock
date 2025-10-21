// Wrapper to read and clear interrupts from the Sub-GHz radio.
// This is a bit of a workaround for now and should eventually
// be replaced.

use kernel::hil::gpio::{Configure, Input, Interrupt, Output};

pub struct NSS {
    pwr: &'static crate::pwr::Pwr,
}

impl NSS {
    pub const fn new(pwr: &'static crate::pwr::Pwr) -> Self {
        NSS { pwr }
    }

    pub fn set(&self) {
        self.pwr.set_nss()
    }

    pub fn clear(&self) {
        self.pwr.clear_nss()
    }
}

impl VirtualGpioReader for NSS {
    fn read(&self) -> bool {
        // self.pwr.is_set_nss()
        unimplemented!()
    }

    fn write(&self, val: u32) {
        if val == 0 {
            self.clear();
        } else {
            self.set();
        }
    }
}

pub struct SubGhzRadioSignals {}

impl SubGhzRadioSignals {
    pub const fn new() -> Self {
        SubGhzRadioSignals {}
    }
}

impl VirtualGpioReader for SubGhzRadioSignals {
    fn read(&self) -> bool {
        // The Sub-Ghz radio interrupt is level triggered
        // and cannot be cleared except by issuing a subghzspi
        // command to the subghz radio. Because of this, we mask
        // the interrupt in the interrupt handler and perform the
        // check here to see if any other interrupts are pending.
        unsafe {
            cortexm4::nvic::next_pending_with_mask((
                core::u128::MAX,
                !(1 << (crate::nvic::RADIO_IRQ % 32)),
            ))
            .map_or(false, |_| true)
        }
    }
    fn write(&self, _val: u32) {
        unimplemented!()
    }
}

pub struct SubGhzRadioBusy {
    pwr: &'static crate::pwr::Pwr,
}

impl SubGhzRadioBusy {
    pub const fn new(pwr: &'static crate::pwr::Pwr) -> Self {
        SubGhzRadioBusy { pwr }
    }

    pub fn is_busy(&self) -> bool {
        self.pwr.is_rfbusys()
    }
}

impl VirtualGpioReader for SubGhzRadioBusy {
    fn read(&self) -> bool {
        self.is_busy()
    }

    fn write(&self, _val: u32) {}
}

pub trait VirtualGpioReader {
    fn read(&self) -> bool;
    fn write(&self, val: u32);
}

pub struct SubGhzRadioVirtualGpio<'a> {
    reader: &'a dyn VirtualGpioReader,
}

impl<'a> SubGhzRadioVirtualGpio<'a> {
    pub fn new(reader: &'a dyn VirtualGpioReader) -> Self {
        SubGhzRadioVirtualGpio { reader }
    }
}

impl<'a> Interrupt<'a> for SubGhzRadioVirtualGpio<'a> {
    fn disable_interrupts(&self) {
        // do nothing
    }

    fn enable_interrupts(&self, _mode: kernel::hil::gpio::InterruptEdge) {
        // do nothing
    }

    fn is_pending(&self) -> bool {
        false
    }

    fn set_client(&self, _client: &'a dyn kernel::hil::gpio::Client) {
        // do nothing
    }
}

impl<'a> Input for SubGhzRadioVirtualGpio<'a> {
    fn read(&self) -> bool {
        let res = self.reader.read();
        res
    }
}

impl<'a> Output for SubGhzRadioVirtualGpio<'a> {
    fn clear(&self) {
        // do nothing
    }

    fn set(&self) {
        // do nothing
    }

    fn toggle(&self) -> bool {
        // do nothing
        false
    }
}

impl<'a> Configure for SubGhzRadioVirtualGpio<'a> {
    fn configuration(&self) -> kernel::hil::gpio::Configuration {
        unimplemented!()
    }

    fn deactivate_to_low_power(&self) {
        unimplemented!()
    }

    fn disable_input(&self) -> kernel::hil::gpio::Configuration {
        unimplemented!()
    }

    fn disable_output(&self) -> kernel::hil::gpio::Configuration {
        unimplemented!()
    }

    fn floating_state(&self) -> kernel::hil::gpio::FloatingState {
        unimplemented!()
    }

    fn set_floating_state(&self, _state: kernel::hil::gpio::FloatingState) {}

    fn is_input(&self) -> bool {
        unimplemented!()
    }

    fn is_output(&self) -> bool {
        unimplemented!()
    }

    fn make_input(&self) -> kernel::hil::gpio::Configuration {
        kernel::hil::gpio::Configuration::Input
    }

    fn make_output(&self) -> kernel::hil::gpio::Configuration {
        unimplemented!()
    }
}
