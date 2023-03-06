#![no_std]
#![forbid(unsafe_code)]
#![deny(clippy::pedantic)]
#![allow(clippy::missing_errors_doc)]

mod registers;

use embedded_hal::i2c::I2c;
use registers::{Control2, Registers};
use settings::Settings;

pub mod settings {
    use enumn::N;

    #[derive(Copy, Clone, Debug, N)]
    pub enum OutputDataRate {
        /// 10Hz
        OSR10 = 0b00,
        /// 50Hz
        OSR50 = 0b01,
        /// 100Hz
        OSR100 = 0b10,
        /// 200Hz
        OSR200 = 0b11,
    }

    #[derive(Copy, Clone, Debug, N)]
    pub enum OverSampleRatio {
        OSR512 = 0b00,
        OSR256 = 0b01,
        OSR128 = 0b10,
        OSR64 = 0b11,
    }

    #[derive(Copy, Clone, Debug, N)]
    pub enum FullScale {
        RNG2G = 0b00,
        RNG8G = 0b01,
    }

    #[derive(Copy, Clone, Debug)]
    pub struct Settings {
        pub odr: OutputDataRate,
        pub osr: OverSampleRatio,
        pub rng: FullScale,
    }

    impl Settings {
        pub(crate) const ADDR: u8 = 0x09;
    }

    impl Into<u8> for Settings {
        fn into(self) -> u8 {
            let mut val = 0;
            val += (self.odr as u8) << 2;
            val += (self.rng as u8) << 4;
            val += (self.osr as u8) << 6;
            // We add 1 to reflect the continuous measurement mode
            val += 1;
            val
        }
    }

    impl From<u8> for Settings {
        fn from(val: u8) -> Self {
            Self {
                odr: OutputDataRate::n((val & 0b1100_0000) >> 6).unwrap(),
                rng: FullScale::n((val & 0b0011_0000) >> 4).unwrap(),
                osr: OverSampleRatio::n((val & 0b0000_1100) >> 2).unwrap(),
            }
        }
    }
}

pub struct QMC8553L<I: I2c> {
    i2c: I,
    standby: bool,
}

impl<I: I2c> QMC8553L<I> {
    const ADDR: u8 = 0x0D;

    /// Initialise the device with the given [`Settings`].
    ///
    /// # Notes
    ///
    /// - As part of this process, perform a sofware reset of the device
    /// - The device will **not** be in "Standby" mode afterwards
    pub fn new(i2c: I, set: Settings) -> Result<(), I::Error> {
        let mut to_ret = Self {
            i2c,
            standby: false,
        };
        to_ret.reset()?;
        to_ret.set_settings(set)?;
        // Enable pointer rollover
        // TODO: if we write interrupts code in the future, we need to enable them here!
        to_ret.set_control2(Control2::ROL_PNT)?;
        Ok(())
    }

    /// Perform a soft reset of the device.
    ///
    /// This **does not** place the device into "Standby" mode!
    pub fn reset(&mut self) -> Result<(), I::Error> {
        self.set_control2(Control2::SOFT_RST)?;
        // Reenable pointer rollover
        self.set_control2(Control2::ROL_PNT)?;
        Ok(())
    }

    /// Set the "Standby" mode on the device to conserve power.
    ///
    /// All interaction with the device afterwards will automatically wake it up.
    pub fn to_standby(&mut self) -> Result<(), I::Error> {
        self.set_standby()?;
        self.standby = true;
        Ok(())
    }

    /// Check if the device is on standby.
    ///
    /// The user should note that this is only tracked in software (otherwise checking the flag
    /// would wake the device up!).
    pub fn on_standby(&self) -> bool {
        self.standby
    }

    /// Check if the device is ready to have data read off it.
    pub fn is_ready(&mut self) -> Result<bool, I::Error> {
        Ok(self.get_status()?.contains(registers::Status::DRDY))
    }

    /// Read all three axes' data off the device.
    ///
    /// You should check with [`Self::is_ready`] before you call this.
    pub fn get_data(&mut self) -> Result<(i16, i16, i16), I::Error> {
        self.read_data()
    }

    /// Get the temperature of the device, in °C.
    ///
    /// Note that the temperature is *not* expected to be absolutely accurate, but *is* expected to be
    /// consistent with itself.
    pub fn get_temp(&mut self) -> Result<i16, I::Error> {
        self.read_reg16s(registers::Register16::TOUT)
    }
}
