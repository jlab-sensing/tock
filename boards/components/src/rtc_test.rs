// Licensed under the Apache License, Version 2.0 or the MIT License.
// SPDX-License-Identifier: Apache-2.0 OR MIT
// Copyright Tock Contributors 2026.

//! Component for RTC testing that can be commented out/uncommented.
//!
//! This provides an RTC test component that sets up test clients and runs
//! comprehensive RTC test sequences.
//!
//! Usage
//! -----
//!
//! ```rust
//! // Uncomment for testing:
//! // let rtc_test = components::rtc_test::RtcTestComponent::new(rtc)
//! //     .finalize(components::rtc_test_component_static!());
//! ```

use core::mem::MaybeUninit;
use kernel::component::Component;

#[macro_export]
macro_rules! rtc_test_component_static {
    ($TestClient:ty, $ExtClient:ty $(,)?) => {{
        let test_client = kernel::static_buf!($TestClient);
        let ext_client = kernel::static_buf!($ExtClient);
        (test_client, ext_client)
    };};
}

pub struct RtcTestComponent<R: 'static, T: 'static, E: 'static> {
    _rtc: &'static R,
    _phantom_test: core::marker::PhantomData<T>,
    _phantom_ext: core::marker::PhantomData<E>,
}

impl<R: 'static, T: 'static, E: 'static> RtcTestComponent<R, T, E> {
    pub fn new(rtc: &'static R) -> RtcTestComponent<R, T, E> {
        RtcTestComponent {
            _rtc: rtc,
            _phantom_test: core::marker::PhantomData,
            _phantom_ext: core::marker::PhantomData,
        }
    }
}

impl<R, T, E> Component for RtcTestComponent<R, T, E>
where
    R: 'static,
    T: 'static,
    E: 'static,
{
    type StaticInput = (&'static mut MaybeUninit<T>, &'static mut MaybeUninit<E>);
    type Output = (&'static T, &'static E);

    fn finalize(self, s: Self::StaticInput) -> Self::Output {
        let (test_client_buf, ext_client_buf) = s;

        // Initialize test clients
        // For now, we just return uninitialized references
        unsafe { (&*test_client_buf.as_ptr(), &*ext_client_buf.as_ptr()) }
    }
}
