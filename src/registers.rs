use bitflags::bitflags;
#[cfg(feature = "defmt")]
use defmt::trace;
use embedded_hal::i2c::I2c;

const SRP_ADDR: u8 = 0x0b;

#[allow(clippy::upper_case_acronyms)]
pub(crate) enum Register16 {
    X = 0x00,
    Y = 0x02,
    Z = 0x04,
    TOUT = 0x07,
}

macro_rules! flag_getter {
    ($funcname:ident -> $reg:ident) => {
        fn $funcname(&mut self) -> Result<$reg, I::Error> {
            #[cfg(feature = "defmt")]
            trace!("Reading flags from {}", FlagRegister::$reg);
            Ok(<$reg>::from_bits_truncate(
                self.read_raw(FlagRegister::$reg as u8)?,
            ))
        }
    };
}
macro_rules! flag_setter {
    ($funcname:ident -> $reg:ident) => {
        fn $funcname(&mut self, val: $reg) -> Result<(), I::Error> {
            #[cfg(feature = "defmt")]
            trace!("Writing flags to {}", FlagRegister::$reg);
            self.write_raw(FlagRegister::$reg as u8, val.bits())
        }
    };
}

pub(crate) enum FlagRegister {
    Status = 0x06,
    Control2 = 0x0a,
}

bitflags! {
    pub struct Status: u8 {
        /// Data Skip.
        ///
        /// If set, all channels of the output data registers are skipped in continuous measurement
        /// mode.
        /// Reset by reading any register.
        const DOR = 0b0100;
        /// Overflow Flag.
        ///
        /// Set if any of the sensor channels is out of range.
        /// Reset when the measurement returns to range.
        /// Note: the range of values is [-32_768, 32767].
        const OVL = 0b0010;
        /// Data Ready.
        ///
        /// Set when all 3 axes' data is ready and loaded in continuous measurement mode.
        /// Set to 0 by reading any register.
        const DRDY = 0b0001;
    }

    pub struct Control2: u8 {
        /// Soft Reset flag.
        const SOFT_RST = 0b1000_0000;
        /// Rolling Pointer Flag.
        ///
        /// Will automatically roll the pointer over when reading from the data registers.
        const ROL_PNT = 0b0100_0000;
        /// Enable interrupts.
        ///
        /// TODO: currently unused.
        const INT_ENB = 0b0001;
    }
}

fn i16_from_le(val: &[u8]) -> i16 {
    bytemuck::cast([val[1], val[0]])
}

pub(crate) trait Registers<I: I2c> {
    const ADDR: u8;

    fn i2c(&mut self) -> &mut I;

    fn read_raw(&mut self, regaddr: u8) -> Result<u8, I::Error> {
        let mut val = [0];

        let to_write = [regaddr];
        self.i2c().write_read(Self::ADDR, &to_write, &mut val)?;
        Ok(val[0])
    }

    fn write_raw(&mut self, regaddr: u8, val: u8) -> Result<(), I::Error> {
        let to_write = [regaddr, val];
        self.i2c().write(Self::ADDR, &to_write)
    }

    fn read_set_reset_period(&mut self) -> Result<u8, I::Error> {
        let raw = self.read_raw(SRP_ADDR)?;
        Ok(bytemuck::cast(raw))
    }

    // Uses pointer rollover to reduce bus load
    fn read_reg16(&mut self, reg: Register16) -> Result<i16, I::Error> {
        let lsb_addr = reg as u8;
        let mut buf = [0; 2];

        self.i2c().write_read(Self::ADDR, &[lsb_addr], &mut buf)?;
        #[cfg(feature = "defmt")]
        trace!("Read value {:?} from register at {}", buf, lsb_addr);
        Ok(i16_from_le(&buf))
    }

    /// Read all 6 data registers off the device.
    ///
    /// Uses pointer rollover to reduce bus load.
    fn read_data(&mut self) -> Result<(i16, i16, i16), I::Error> {
        let addr = Register16::X as u8;
        let mut buf = [0; 6];

        self.i2c().write_read(Self::ADDR, &[addr], &mut buf)?;
        #[cfg(feature = "defmt")]
        trace!("Read raw value {:?} from all axis registers", buf);

        Ok((
            i16_from_le(&buf[0..2]),
            i16_from_le(&buf[2..4]),
            i16_from_le(&buf[4..6]),
        ))
    }

    fn write_set_reset_period(&mut self, val: u8) -> Result<(), I::Error> {
        self.write_raw(SRP_ADDR, bytemuck::cast(val))
    }

    flag_getter! { get_control2 -> Control2 }
    flag_getter! { get_status -> Status }
    flag_setter! { set_control2 -> Control2 }
}

impl<I: I2c> Registers<I> for crate::QMC8553L<I> {
    const ADDR: u8 = 0x0D;

    fn i2c(&mut self) -> &mut I {
        // We set it off standby since we assume the I2C will be used
        // TODO: is this ok?
        self.standby = false;
        &mut self.i2c
    }
}
