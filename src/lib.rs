//! Control and read from the QMC8553L magnetometer.
//!
//! To get started, take a look at [`QMC8553L::new`].
#![no_std]
#![forbid(unsafe_code)]
#![deny(clippy::pedantic)]
#![allow(clippy::missing_errors_doc)]
#![warn(missing_docs)]

// TODO: SET/RESET register access
// TODO: interrupts
// TODO: testing

mod registers;

#[cfg(feature = "defmt")]
use defmt::{debug, info, warn};
use embedded_hal::i2c::I2c;
use registers::Registers;
use settings::Settings;

/// Settings for the device.
pub mod settings {
    #[cfg(feature = "defmt")]
    use defmt::Format;
    use enumn::N;

    // TODO: review defaults

    /// The Output Data Rate of the device.
    ///
    /// Controls the frequency at which reads can be made.
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Default, N)]
    #[cfg_attr(feature = "defmt", derive(Format))]
    pub enum OutputDataRate {
        /// 10Hz
        #[default]
        OSR10 = 0b00,
        /// 50Hz
        OSR50 = 0b01,
        /// 100Hz
        OSR100 = 0b10,
        /// 200Hz
        OSR200 = 0b11,
    }

    /// The Oversample Ratio of the device.
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Default, N)]
    #[cfg_attr(feature = "defmt", derive(Format))]
    #[allow(missing_docs)]
    pub enum OverSampleRatio {
        OSR512 = 0b00,
        OSR256 = 0b01,
        OSR128 = 0b10,
        #[default]
        OSR64 = 0b11,
    }

    #[derive(Copy, Clone, Debug, PartialEq, Eq, Default, N)]
    #[cfg_attr(feature = "defmt", derive(Format))]
    pub enum FullScale {
        #[default]
        RNG2G = 0b00,
        RNG8G = 0b01,
    }

    #[allow(missing_docs)]
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
    #[cfg_attr(feature = "defmt", derive(Format))]
    pub struct Settings {
        pub odr: OutputDataRate,
        pub osr: OverSampleRatio,
        pub rng: FullScale,
    }

    impl Settings {
        pub(crate) const ADDR: u8 = 0x09;
    }

    impl From<Settings> for u8 {
        fn from(set: Settings) -> Self {
            let mut val = 0;
            val += (set.odr as u8) << 2;
            val += (set.rng as u8) << 4;
            val += (set.osr as u8) << 6;
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

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn sanity() {
            let set = Settings::default();

            let intermediate_val: u8 = set.into();

            assert_eq!(<u8 as Into<Settings>>::into(intermediate_val), set);
        }
    }
}

/// An axis of the sensor.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[allow(missing_docs)]
pub enum Axis {
    X,
    Y,
    Z,
}

impl From<Axis> for registers::Register16 {
    fn from(value: Axis) -> Self {
        match value {
            Axis::X => registers::Register16::X,
            Axis::Y => registers::Register16::Y,
            Axis::Z => registers::Register16::Z,
        }
    }
}

/// The QMC8553L magnetometer.
///
/// Use [`QMC8553L::read`] and [`QMC8553L::read_all`] to get the sensor data from the device:
///
/// ```no_run
/// # fn main () {
/// # let mock_i2c = embedded_hal_mock::i2c::Mock::new(&[]);
/// use qmc5883l::{QMC8553L, Axis, settings::Settings};
/// let mut mag = QMC8553L::new(
/// #   mock_i2c,
///     // i2c setup ...
///     Settings::default(),
/// ).unwrap();
///
/// println!("X axis field strength: {:?}", mag.read(Axis::X).unwrap());
/// println!("Full field strength: {:?}", mag.read_all().unwrap());
/// # }
/// ```
pub struct QMC8553L<I: I2c> {
    i2c: I,
    standby: bool,
    // TODO: cache settings here?
    // we should be able to (in order to save bus throughput) since we always explicitly set them
    // on initialisation, and can just cache them then
}

impl<I: I2c> QMC8553L<I> {
    /// Initialise the device with the given [`Settings`].
    ///
    /// # Notes
    ///
    /// - As part of this process, perform a sofware reset of the device
    /// - The device will **not** be in "Standby" mode afterwards
    pub fn new(i2c: I, set: Settings) -> Result<Self, I::Error> {
        let mut to_ret = Self {
            i2c,
            standby: false,
        };
        to_ret.reset()?;
        to_ret.change_settings(set)?;
        Ok(to_ret)
    }

    /// Perform a soft reset of the device.
    ///
    /// This **does not** place the device into "Standby" mode!
    pub fn reset(&mut self) -> Result<(), I::Error> {
        use registers::Control2;
        #[cfg(feature = "defmt")]
        debug!("Resetting QMC8553L magnetometer");
        self.set_control2(Control2::SOFT_RST)?;
        // TODO: delay period?
        // Reenable pointer rollover
        #[cfg(feature = "defmt")]
        debug!("Enabling pointer rollover");
        self.set_control2(Control2::ROL_PNT)?;
        // TODO: if we write interrupts code in the future, we need to enable them here!
        Ok(())
    }

    /// Set the "Standby" mode on the device to conserve power.
    ///
    /// All interaction with the device afterwards will automatically wake it up.
    pub fn to_standby(&mut self) -> Result<(), I::Error> {
        let mut set_val: u8 = self.settings()?.into();
        // unset the continuous measurement bit
        set_val &= 0b1111_1100;
        #[cfg(feature = "defmt")]
        debug!("Sending QMC5883L to standby mode");
        self.write_raw(Settings::ADDR, set_val)?;
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
    pub fn read_all(&mut self) -> Result<(i16, i16, i16), I::Error> {
        self.read_data()
    }

    /// Read a particular axis' data.
    ///
    /// You should check with [`Self::is_ready`] before you call this.
    pub fn read(&mut self, axis: Axis) -> Result<i16, I::Error> {
        self.read_reg16(axis.into())
    }

    /// Get the temperature of the device, in °C.
    ///
    /// Note that the temperature is *not* expected to be absolutely accurate, but *is* expected to be
    /// consistent with itself.
    pub fn get_temp(&mut self) -> Result<i16, I::Error> {
        self.read_reg16(registers::Register16::TOUT)
    }

    /// Get the currently set [`Settings`] on the device.
    pub fn settings(&mut self) -> Result<Settings, I::Error> {
        let val = self.read_raw(Settings::ADDR)?;
        Ok(Settings::from(val))
    }

    /// Change the current [`Settings`] on the device.
    pub fn change_settings(&mut self, set: Settings) -> Result<(), I::Error> {
        #[cfg(feature = "defmt")]
        debug!("Applying {:?} to magnetometer", set);
        self.write_raw(Settings::ADDR, set.into())
    }
}
