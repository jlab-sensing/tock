#![no_std]

pub use stm32wle5xx::{chip, clocks, gpio, nvic, rcc, usart};

pub mod chip_specs;
pub mod interrupt_service;
use cortexm4f::{unhandled_interrupt, CortexM4F, CortexMVariant};

// STM32F446xx has total of 97 interrupts
// Extracted from `CMSIS/Device/ST/STM32F4xx/Include/stm32f446xx.h`
// NOTE: There are missing IRQn between 0 and 96
#[cfg_attr(all(target_arch = "arm", target_os = "none"), link_section = ".irqs")]
// `used` ensures that the symbol is kept until the final binary. However, as of
// May 2020, due to the compilation process, there must be some other compiled
// code here to make sure the object file is kept around. That means at minimum
// there must be an `init()` function here so that compiler does not just ignore
// the `IRQS` object. See https://github.com/rust-lang/rust/issues/56639 for a
// related discussion.
#[cfg_attr(all(target_arch = "arm", target_os = "none"), used)]
pub static IRQS: [unsafe extern "C" fn(); 62] = [
    CortexM4F::GENERIC_ISR, // WWDG (0)
    CortexM4F::GENERIC_ISR, // PVD (1)
    CortexM4F::GENERIC_ISR, // TAMP_STAMP (2)
    CortexM4F::GENERIC_ISR, // RTC_WKUP (3)
    CortexM4F::GENERIC_ISR, // FLASH (4)
    CortexM4F::GENERIC_ISR, // RCC (5)
    CortexM4F::GENERIC_ISR, // EXTI0 (6)
    CortexM4F::GENERIC_ISR, // EXTI1 (7)
    CortexM4F::GENERIC_ISR, // EXTI2 (8)
    CortexM4F::GENERIC_ISR, // EXTI3 (9)
    CortexM4F::GENERIC_ISR, // EXTI4 (10)
    CortexM4F::GENERIC_ISR, // DMA1_Stream0 (11)
    CortexM4F::GENERIC_ISR, // DMA1_Stream1 (12)
    CortexM4F::GENERIC_ISR, // DMA1_Stream2 (13)
    CortexM4F::GENERIC_ISR, // DMA1_Stream3 (14)
    CortexM4F::GENERIC_ISR, // DMA1_Stream4 (15)
    CortexM4F::GENERIC_ISR, // DMA1_Stream5 (16)
    CortexM4F::GENERIC_ISR, // DMA1_Stream6 (17)
    CortexM4F::GENERIC_ISR, // ADC (18)
    CortexM4F::GENERIC_ISR, // DAC (19)
    unhandled_interrupt,    // RESERVED (20)
    CortexM4F::GENERIC_ISR, // COMP (21)
    CortexM4F::GENERIC_ISR, // EXTI9_5 (22)
    CortexM4F::GENERIC_ISR, // TIM1_BRK (23)
    CortexM4F::GENERIC_ISR, // TIM1_UP (24)
    CortexM4F::GENERIC_ISR, // TIM1_TRG_COM (25)
    CortexM4F::GENERIC_ISR, // TIM1_CC (26)
    CortexM4F::GENERIC_ISR, // TIM2 (27)
    CortexM4F::GENERIC_ISR, // TIM16 (28)
    CortexM4F::GENERIC_ISR, // TIM17 (29)
    CortexM4F::GENERIC_ISR, // I2C1_EV (30)
    CortexM4F::GENERIC_ISR, // I2C1_ER (31)
    CortexM4F::GENERIC_ISR, // I2C2_EV (32)
    CortexM4F::GENERIC_ISR, // I2C2_ER (33)
    CortexM4F::GENERIC_ISR, // SPI1 (34)
    CortexM4F::GENERIC_ISR, // SPI2S2 (35)
    CortexM4F::GENERIC_ISR, // USART1 (36)
    CortexM4F::GENERIC_ISR, // USART2 (37)
    CortexM4F::GENERIC_ISR, // LPUART1 (38)
    CortexM4F::GENERIC_ISR, // LPTIM1 (39)
    CortexM4F::GENERIC_ISR, // LPTIM2 (40)
    CortexM4F::GENERIC_ISR, // EXTI15_10 (41)
    CortexM4F::GENERIC_ISR, // RTC_ALARM (42)
    CortexM4F::GENERIC_ISR, // LPTIM3 (43)
    unhandled_interrupt,    // RESERVED (44)
    unhandled_interrupt,    // RESERVED (45)
    unhandled_interrupt,    // RESERVED (46)
    CortexM4F::GENERIC_ISR, // HSEM (47)
    CortexM4F::GENERIC_ISR, // I2C3_EV (48)
    CortexM4F::GENERIC_ISR, // I2C3_ER (49)
    CortexM4F::GENERIC_ISR, // RADIO_IRQ_BUSY (50)
    CortexM4F::GENERIC_ISR, // AES (51)
    CortexM4F::GENERIC_ISR, // TRNG (52)
    CortexM4F::GENERIC_ISR, // PKA (53)
    CortexM4F::GENERIC_ISR, // DMA2_Stream0 (54)
    CortexM4F::GENERIC_ISR, // DMA2_Stream1 (55)
    CortexM4F::GENERIC_ISR, // DMA2_Stream2 (56)
    CortexM4F::GENERIC_ISR, // DMA2_Stream3 (57)
    CortexM4F::GENERIC_ISR, // DMA2_Stream4 (58)
    CortexM4F::GENERIC_ISR, // DMA2_Stream5 (59)
    CortexM4F::GENERIC_ISR, // DMA2_Stream6 (60)
    CortexM4F::GENERIC_ISR, // DMAMUX1_OVR (61)
];

pub unsafe fn init() {
    stm32wle5xx::init();
}
