// Licensed under the Apache License, Version 2.0 or the MIT License.
// SPDX-License-Identifier: Apache-2.0 OR MIT
// Copyright Tock Contributors 2022.

use crate::clocks::Stm32f4Clocks;
use crate::rcc::{APBPrescaler, Rcc, RtcClockSource};
use kernel::platform::chip::ClockInterface;

pub struct PeripheralClock<'a> {
    pub clock: PeripheralClockType,
    clocks: &'a dyn Stm32f4Clocks,
}

/// Bus + Clock name for the peripherals
pub enum PeripheralClockType {
    AHB1(HCLK1),
    AHB2(HCLK2),
    AHB3(HCLK3),
    APB1(PCLK1),
    APB2(PCLK2),
    RTC,
}

/// Peripherals clocked by HCLK1
pub enum HCLK1 {
    DMA1,
    DMA2,
    DMAMUX1,
    CRC,
}

/// Peripherals clocked by HCLK3
pub enum HCLK3 {
    PKA,
    AES,
    RNG,
    HSEM,
    FLASH,
}

/// Peripherals clocked by HCLK2
pub enum HCLK2 {
    GPIOA,
    GPIOB,
    GPIOC,
    GPIOH,
}

/// Peripherals clocked by PCLK1
pub enum PCLK1 {
    TIM2,
    RTCAPB,
    WWDG,
    SPI2S2,
    USART2,
    I2C1,
    I2C2,
    I2C3,
    DAC,
    LPTIM1,
    LPUART1,
    LPTIM2,
    LPTIM3,
}

/// Peripherals clocked by PCLK2
pub enum PCLK2 {
    ADC,
    TIM1,
    SPI1,
    USART1,
    TIM16,
    TIM17,
}

impl<'a> PeripheralClock<'a> {
    pub const fn new(clock: PeripheralClockType, clocks: &'a dyn Stm32f4Clocks) -> Self {
        Self { clock, clocks }
    }

    pub fn configure_rng_clock(&self) {
        self.clocks.get_rcc().configure_rng_clock();
    }

    pub fn get_frequency(&self) -> u32 {
        #[inline(always)]
        fn tim_freq(rcc: &Rcc, hclk_freq: usize, prescaler: APBPrescaler) -> usize {
            // Reference Manual RM0090 section 6.2
            // When TIMPRE bit of the RCC_DCKCFGR register is reset, if APBx prescaler is 1, then
            // TIMxCLK = PCLKx, otherwise TIMxCLK = 2x PCLKx.
            // When TIMPRE bit in the RCC_DCKCFGR register is set, if APBx prescaler is 1,2 or 4,
            // then TIMxCLK = HCLK, otherwise TIMxCLK = 4x PCLKx.
            if !rcc.is_enabled_tim_pre() {
                match prescaler {
                    APBPrescaler::DivideBy1 | APBPrescaler::DivideBy2 => hclk_freq,
                    _ => hclk_freq / usize::from(prescaler) * 2,
                }
            } else {
                match prescaler {
                    APBPrescaler::DivideBy1 | APBPrescaler::DivideBy2 | APBPrescaler::DivideBy4 => {
                        hclk_freq
                    }
                    _ => hclk_freq / usize::from(prescaler) * 4,
                }
            }
        }
        let rcc = self.clocks.get_rcc();
        let hclk_freq = self.clocks.get_ahb_frequency();
        match self.clock {
            PeripheralClockType::AHB1(_)
            | PeripheralClockType::AHB2(_)
            | PeripheralClockType::AHB3(_) => hclk_freq as u32,
            PeripheralClockType::APB1(ref v) => {
                let prescaler = rcc.get_apb1_prescaler();
                match v {
                    PCLK1::TIM2 => tim_freq(rcc, hclk_freq, prescaler) as u32,
                    _ => (hclk_freq / usize::from(prescaler)) as u32,
                }
            }
            PeripheralClockType::APB2(_) => {
                let prescaler = rcc.get_apb2_prescaler();
                (hclk_freq / usize::from(prescaler)) as u32
            }
            //TODO: implement clock frequency retrieval for RTC and PWR peripherals
            PeripheralClockType::RTC => todo!(),
        }
    }
}

impl<'a> ClockInterface for PeripheralClock<'a> {
    fn is_enabled(&self) -> bool {
        let rcc = self.clocks.get_rcc();
        match self.clock {
            PeripheralClockType::AHB1(ref v) => match v {
                HCLK1::DMA1 => rcc.is_enabled_dma1_clock(),
                HCLK1::DMA2 => rcc.is_enabled_dma2_clock(),
                _ => unimplemented!(),
            },
            PeripheralClockType::AHB2(ref v) => match v {
                _ => unimplemented!(),
            },
            PeripheralClockType::AHB3(ref v) => match v {
                _ => unimplemented!(),
            },
            PeripheralClockType::APB1(ref v) => match v {
                PCLK1::TIM2 => rcc.is_enabled_tim2_clock(),
                PCLK1::USART2 => rcc.is_enabled_usart2_clock(),
                PCLK1::I2C1 => rcc.is_enabled_i2c1_clock(),
                PCLK1::DAC => rcc.is_enabled_dac_clock(),
                _ => unimplemented!(),
            },
            PeripheralClockType::APB2(ref v) => match v {
                PCLK2::USART1 => rcc.is_enabled_usart1_clock(),
                _ => unimplemented!(),
            },
            PeripheralClockType::RTC => rcc.is_enabled_rtc_clock(),
        }
    }

    fn enable(&self) {
        let rcc = self.clocks.get_rcc();
        match self.clock {
            PeripheralClockType::AHB1(ref v) => match v {
                HCLK1::DMA1 => {
                    rcc.enable_dma1_clock();
                }
                HCLK1::DMA2 => {
                    rcc.enable_dma2_clock();
                }
                _ => unimplemented!(),
            },
            PeripheralClockType::AHB2(ref v) => match v {
                _ => unimplemented!(),
            },
            PeripheralClockType::AHB3(ref v) => match v {
                _ => unimplemented!(),
            },
            PeripheralClockType::APB1(ref v) => match v {
                PCLK1::TIM2 => {
                    rcc.enable_tim2_clock();
                }
                PCLK1::USART2 => {
                    rcc.enable_usart2_clock();
                }
                PCLK1::I2C1 => {
                    rcc.enable_i2c1_clock();
                }
                PCLK1::DAC => {
                    rcc.enable_dac_clock();
                }
                _ => unimplemented!(),
            },
            PeripheralClockType::APB2(ref v) => match v {
                PCLK2::USART1 => {
                    rcc.enable_usart1_clock();
                }
                _ => unimplemented!(),
            },
            PeripheralClockType::RTC => rcc.enable_rtc_clock(RtcClockSource::LSI),
        }
    }

    fn disable(&self) {
        let rcc = self.clocks.get_rcc();
        match self.clock {
            PeripheralClockType::AHB1(ref v) => match v {
                HCLK1::DMA1 => {
                    rcc.disable_dma1_clock();
                }
                HCLK1::DMA2 => {
                    rcc.disable_dma2_clock();
                }
                _ => unimplemented!(),
            },
            PeripheralClockType::AHB2(ref v) => match v {
                _ => unimplemented!(),
            },
            PeripheralClockType::AHB3(ref v) => match v {
                _ => unimplemented!(),
            },
            PeripheralClockType::APB1(ref v) => match v {
                PCLK1::TIM2 => {
                    rcc.disable_tim2_clock();
                }
                PCLK1::USART2 => {
                    rcc.disable_usart2_clock();
                }
                PCLK1::DAC => {
                    rcc.disable_dac_clock();
                }
                _ => unimplemented!(),
            },
            PeripheralClockType::APB2(ref v) => match v {
                PCLK2::USART1 => {
                    rcc.disable_usart1_clock();
                }
                _ => unimplemented!(),
            },
            PeripheralClockType::RTC => rcc.disable_rtc_clock(),
        }
    }
}
