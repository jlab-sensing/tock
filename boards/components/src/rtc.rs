// Licensed under the Apache License, Version 2.0 or the MIT License.
// SPDX-License-Identifier: Apache-2.0 OR MIT
// Copyright Tock Contributors 2026.

//! Component for RTC (Real-Time Clock) initialization.
//!
//! This provides a standard RTC component for production use that encapsulates
//! peripheral initialization and client setup. NVIC interrupt enables should be
//! done in the board's setup_peripherals() or create_peripherals() function.
//!
//! Usage
//! -----
//!
//! ```rust
//! let rtc = components::rtc::RtcComponent::new(&peripherals.rtc)
//!     .finalize(components::rtc_component_static!());
//! ```

use core::mem::MaybeUninit;
use kernel::component::Component;

#[macro_export]
macro_rules! rtc_component_static {
    () => {{
        kernel::static_buf!(())
    };};
}

pub struct RtcComponent<R: 'static> {
    rtc: &'static R,
}

impl<R: 'static> RtcComponent<R> {
    pub fn new(rtc: &'static R) -> RtcComponent<R> {
        RtcComponent { rtc }
    }
}

impl<R: 'static> Component for RtcComponent<R> {
    type StaticInput = &'static mut MaybeUninit<()>;
    type Output = &'static R;

    fn finalize(self, _s: Self::StaticInput) -> Self::Output {
        // setup_peripherals() or create_peripherals() function
        self.rtc
    }
}
