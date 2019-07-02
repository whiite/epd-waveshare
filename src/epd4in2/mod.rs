//! A simple Driver for the Waveshare 4.2" E-Ink Display via SPI
//!
//! The other Waveshare E-Ink Displays should be added later on
//!
//! Build with the help of documentation/code from [Waveshare](https://www.waveshare.com/wiki/4.2inch_e-Paper_Module),
//! [Ben Krasnows partial Refresh tips](https://benkrasnow.blogspot.de/2017/10/fast-partial-refresh-on-42-e-paper.html) and
//! the driver documents in the `pdfs`-folder as orientation.
//!
//! This driver was built using [`embedded-hal`] traits.
//!
//! [`embedded-hal`]: https://docs.rs/embedded-hal/~0.1
//!
//! # Requirements
//!
//! ### SPI
//!
//! - MISO is not connected/available
//! - SPI_MODE_0 is used (CPHL = 0, CPOL = 0)
//! - 8 bits per word, MSB first
//! - Max. Speed tested was 8Mhz but more should be possible
//!
//! ### Other....
//!
//! - Buffersize: Wherever a buffer is used it always needs to be of the size: `width / 8 * length`,
//!   where width and length being either the full e-ink size or the partial update window size
//!
//! # Examples
//!
//! ```ignore
//! let mut epd4in2 = EPD4in2::new(spi, cs, busy, dc, rst, delay).unwrap();
//!
//! let mut buffer =  [0u8, epd4in2.get_width() / 8 * epd4in2.get_height()];
//!
//! // draw something into the buffer
//!
//! epd4in2.display_and_transfer_buffer(buffer, None);
//!
//! // wait and look at the image
//!
//! epd4in2.clear_frame(None);
//!
//! epd4in2.sleep();
//! ```
//!
//!
//!
//! BE CAREFUL! The screen can get ghosting/burn-ins through the Partial Fast Update Drawing.

use embedded_hal::{
    blocking::{delay::*, spi::Write},
    digital::v2::*,
};
use crate::Error;

use crate::interface::DisplayInterface;
use crate::traits::{InternalWiAdditions, RefreshLUT, WaveshareDisplay};

//The Lookup Tables for the Display
mod constants;
use crate::epd4in2::constants::*;

pub const WIDTH: u32 = 400;
pub const HEIGHT: u32 = 300;
pub const DEFAULT_BACKGROUND_COLOR: Color = Color::White;
const IS_BUSY_LOW: bool = true;

use crate::color::Color;

pub(crate) mod command;
use self::command::Command;

#[cfg(feature = "graphics")]
mod graphics;
#[cfg(feature = "graphics")]
pub use self::graphics::Display4in2;

/// EPD4in2 driver
///
pub struct EPD4in2<SPI, CS, BUSY, DC, RST> {
    /// Connection Interface
    di: DisplayInterface<SPI, CS, BUSY, DC, RST>,
    /// Background Color
    color: Color,
    /// Refresh LUT
    refresh: RefreshLUT,
}

impl<SPI, CS, BUSY, DC, RST, SpiE, PinRE, PinWE> InternalWiAdditions<SPI, CS, BUSY, DC, RST, SpiE, PinRE, PinWE>
    for EPD4in2<SPI, CS, BUSY, DC, RST>
where
    SPI: Write<u8, Error = SpiE>,
    CS: OutputPin<Error = PinWE>,
    BUSY: InputPin<Error = PinRE>,
    DC: OutputPin<Error = PinWE>,
    RST: OutputPin<Error = PinWE>,
{
    type Error = Error<SpiE, PinRE, PinWE>;
    
    fn init<DELAY: DelayMs<u8>>(
        &mut self,
        spi: &mut SPI,
        delay: &mut DELAY,
    ) -> Result<(), Self::Error> {
        // reset the device
        self.di.reset(delay);

        // set the power settings
        self.di.cmd_with_data(
            spi,
            Command::POWER_SETTING,
            &[0x03, 0x00, 0x2b, 0x2b, 0xff],
        )?;

        // start the booster
        self.di
            .cmd_with_data(spi, Command::BOOSTER_SOFT_START, &[0x17, 0x17, 0x17])?;

        // power on
        self.di.cmd(spi, Command::POWER_ON)?;
        delay.delay_ms(5);
        self.wait_until_idle();

        // set the panel settings
        self.di.cmd_with_data(spi, Command::PANEL_SETTING, &[0x3F])?;

        // Set Frequency, 200 Hz didn't work on my board
        // 150Hz and 171Hz wasn't tested yet
        // TODO: Test these other frequencies
        // 3A 100HZ   29 150Hz 39 200HZ  31 171HZ DEFAULT: 3c 50Hz
        self.di.cmd_with_data(spi, Command::PLL_CONTROL, &[0x3A])?;

        self.set_lut(spi, None)?;

        self.wait_until_idle();
        Ok(())
    }
}

impl<SPI, CS, BUSY, DC, RST, SpiE, PinRE, PinWE> WaveshareDisplay<SPI, CS, BUSY, DC, RST, SpiE, PinRE, PinWE>
    for EPD4in2<SPI, CS, BUSY, DC, RST>
where
    SPI: Write<u8, Error = SpiE>,
    CS: OutputPin<Error = PinWE>,
    BUSY: InputPin<Error = PinRE>,
    DC: OutputPin<Error = PinWE>,
    RST: OutputPin<Error = PinWE>,
{
    type Error = Error<SpiE, PinRE, PinWE>;
    
    /// Creates a new driver from a SPI peripheral, CS Pin, Busy InputPin, DC
    ///
    /// This already initialises the device. That means [init()](init()) isn't needed directly afterwards
    ///
    /// # Example
    ///
    /// ```ignore
    /// //buffer = some image data;
    ///
    /// let mut epd4in2 = EPD4in2::new(spi, cs, busy, dc, rst, delay);
    ///
    /// epd4in2.display_and_transfer_frame(buffer, None);
    ///
    /// epd4in2.sleep();
    /// ```
    fn new<DELAY: DelayMs<u8>>(
        spi: &mut SPI,
        cs: CS,
        busy: BUSY,
        dc: DC,
        rst: RST,
        delay: &mut DELAY,
    ) -> Result<Self, SPI::Error> {
        let di = DisplayInterface::new(cs, busy, dc, rst);
        let color = DEFAULT_BACKGROUND_COLOR;

        let mut epd = EPD4in2 {
            di,
            color,
            refresh: RefreshLUT::FULL,
        };

        epd.init(spi, delay)?;

        Ok(epd)
    }

    fn wake_up<DELAY: DelayMs<u8>>(
        &mut self,
        spi: &mut SPI,
        delay: &mut DELAY,
    ) -> Result<(), SPI::Error> {
        self.init(spi, delay)
    }

    fn sleep(&mut self, spi: &mut SPI) -> Result<(), SPI::Error> {
        self.di
            .cmd_with_data(spi, Command::VCOM_AND_DATA_INTERVAL_SETTING, &[0x17])?; //border floating
        self.di.cmd(spi, Command::VCM_DC_SETTING)?; // VCOM to 0V
        self.di.cmd(spi, Command::PANEL_SETTING)?;

        self.di.cmd(spi, Command::POWER_SETTING)?; //VG&VS to 0V fast
        for _ in 0..4 {
            self.di.data(spi, &[0x00])?;
        }

        self.di.cmd(spi, Command::POWER_OFF)?;
        self.wait_until_idle();
        self.di
            .cmd_with_data(spi, Command::DEEP_SLEEP, &[0xA5])?;

        self.wait_until_idle();
        Ok(())
    }

    fn update_frame(&mut self, spi: &mut SPI, buffer: &[u8]) -> Result<(), SPI::Error> {
        let color_value = self.color.get_byte_value();

        self.send_resolution(spi)?;

        self.di
            .cmd_with_data(spi, Command::VCM_DC_SETTING, &[0x12])?;

        //VBDF 17|D7 VBDW 97  VBDB 57  VBDF F7  VBDW 77  VBDB 37  VBDR B7
        self.di
            .cmd_with_data(spi, Command::VCOM_AND_DATA_INTERVAL_SETTING, &[0x97])?;

        self.di
            .cmd(spi, Command::DATA_START_TRANSMISSION_1)?;
        self.di
            .data_x_times(spi, color_value, WIDTH / 8 * HEIGHT)?;

        self.di
            .cmd_with_data(spi, Command::DATA_START_TRANSMISSION_2, buffer)?;

        self.wait_until_idle();
        Ok(())
    }

    fn update_partial_frame(
        &mut self,
        spi: &mut SPI,
        buffer: &[u8],
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) -> Result<(), SPI::Error> {
        if buffer.len() as u32 != width / 8 * height {
            //TODO: panic!! or sth like that
            //return Err("Wrong buffersize");
        }

        self.di.cmd(spi, Command::PARTIAL_IN)?;
        self.di.cmd(spi, Command::PARTIAL_WINDOW)?;
        self.di.data(spi, &[(x >> 8) as u8])?;
        let tmp = x & 0xf8;
        self.di.data(spi, &[tmp as u8])?; // x should be the multiple of 8, the last 3 bit will always be ignored
        let tmp = tmp + width - 1;
        self.di.data(spi, &[(tmp >> 8) as u8])?;
        self.di.data(spi, &[(tmp | 0x07) as u8])?;

        self.di.data(spi, &[(y >> 8) as u8])?;
        self.di.data(spi, &[y as u8])?;

        self.di.data(spi, &[((y + height - 1) >> 8) as u8])?;
        self.di.data(spi, &[(y + height - 1) as u8])?;

        self.di.data(spi, &[0x01])?; // Gates scan both inside and outside of the partial window. (default)

        //TODO: handle dtm somehow
        let is_dtm1 = false;
        if is_dtm1 {
            self.di.cmd(spi, Command::DATA_START_TRANSMISSION_1)? //TODO: check if data_start transmission 1 also needs "old"/background data here
        } else {
            self.di.cmd(spi, Command::DATA_START_TRANSMISSION_2)?
        }

        self.di.data(spi, buffer)?;

        self.di.cmd(spi, Command::PARTIAL_OUT)?;

        self.wait_until_idle();
        Ok(())
    }

    fn display_frame(&mut self, spi: &mut SPI) -> Result<(), SPI::Error> {
        self.di.cmd(spi, Command::DISPLAY_REFRESH)?;

        self.wait_until_idle();
        Ok(())
    }

    fn clear_frame(&mut self, spi: &mut SPI) -> Result<(), SPI::Error> {
        self.send_resolution(spi)?;

        let color_value = self.color.get_byte_value();

        self.di
            .cmd(spi, Command::DATA_START_TRANSMISSION_1)?;
        self.di
            .data_x_times(spi, color_value, WIDTH / 8 * HEIGHT)?;

        self.di
            .cmd(spi, Command::DATA_START_TRANSMISSION_2)?;
        self.di
            .data_x_times(spi, color_value, WIDTH / 8 * HEIGHT)?;

        self.wait_until_idle();
        Ok(())
    }

    fn set_background_color(&mut self, color: Color) {
        self.color = color;
    }

    fn background_color(&self) -> &Color {
        &self.color
    }

    fn width(&self) -> u32 {
        WIDTH
    }

    fn height(&self) -> u32 {
        HEIGHT
    }

    fn set_lut(
        &mut self,
        spi: &mut SPI,
        refresh_rate: Option<RefreshLUT>,
    ) -> Result<(), SPI::Error> {
        if let Some(refresh_lut) = refresh_rate {
            self.refresh = refresh_lut;
        }
        match self.refresh {
            RefreshLUT::FULL => {
                self.set_lut_helper(spi, &LUT_VCOM0, &LUT_WW, &LUT_BW, &LUT_WB, &LUT_BB)
            }
            RefreshLUT::QUICK => self.set_lut_helper(
                spi,
                &LUT_VCOM0_QUICK,
                &LUT_WW_QUICK,
                &LUT_BW_QUICK,
                &LUT_WB_QUICK,
                &LUT_BB_QUICK,
            ),
        }
    }

    fn is_busy(&self) -> bool {
        self.di.is_busy(IS_BUSY_LOW)
    }
}

impl<SPI, CS, BUSY, DC, RST, SpiE, PinRE, PinWE> EPD4in2<SPI, CS, BUSY, DC, RST>
where
    SPI: Write<u8, Error = SpiE>,
    CS: OutputPin<Error = PinWE>,
    BUSY: InputPin<Error = PinRE>,
    DC: OutputPin<Error = PinWE>,
    RST: OutputPin<Error = PinWE>,
{
    

    fn wait_until_idle(&mut self) {
        self.di.wait_until_idle(IS_BUSY_LOW)
    }

    fn send_resolution(&mut self, spi: &mut SPI) -> Result<(), SPI::Error> {
        let w = self.width();
        let h = self.height();

        self.di.cmd(spi, Command::RESOLUTION_SETTING)?;
        self.di.data(spi, &[(w >> 8) as u8])?;
        self.di.data(spi, &[w as u8])?;
        self.di.data(spi, &[(h >> 8) as u8])?;
        self.di.data(spi, &[h as u8])
    }

    fn set_lut_helper(
        &mut self,
        spi: &mut SPI,
        lut_vcom: &[u8],
        lut_ww: &[u8],
        lut_bw: &[u8],
        lut_wb: &[u8],
        lut_bb: &[u8],
    ) -> Result<(), SPI::Error> {
        // LUT VCOM
        self.di.cmd_with_data(spi, Command::LUT_FOR_VCOM, lut_vcom)?;

        // LUT WHITE to WHITE
        self.di.cmd_with_data(spi, Command::LUT_WHITE_TO_WHITE, lut_ww)?;

        // LUT BLACK to WHITE
        self.di.cmd_with_data(spi, Command::LUT_BLACK_TO_WHITE, lut_bw)?;

        // LUT WHITE to BLACK
        self.di.cmd_with_data(spi, Command::LUT_WHITE_TO_BLACK, lut_wb)?;

        // LUT BLACK to BLACK
        self.di.cmd_with_data(spi, Command::LUT_BLACK_TO_BLACK, lut_bb)?;

        self.di.wait_until_idle(IS_BUSY_LOW);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epd_size() {
        assert_eq!(WIDTH, 400);
        assert_eq!(HEIGHT, 300);
        assert_eq!(DEFAULT_BACKGROUND_COLOR, Color::White);
    }
}
