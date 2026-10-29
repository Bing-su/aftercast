//! Session-owned ACT color scalar settings; no operating-system theme is changed.
use crate::{Error, HostHandle, runtime::Request};
use bytes::{BufMut, BytesMut};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ColorScalars {
    pub invert: bool,
    /// Hundredths in -100..=100; ACT exposes this as a decimal in -1..=1.
    pub luminosity: i8,
    /// Hundredths in -100..=100.
    pub saturation: i8,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorControls {
    /// Optional opaque RGB; None represents the app's disabled highlight.
    pub ally: Option<u32>,
    pub personal: Option<u32>,
}
impl Default for ColorControls {
    fn default() -> Self {
        Self {
            ally: Some(0x228b22),
            personal: Some(0xc8ffc8),
        }
    }
}
impl HostHandle {
    pub async fn set_color_controls(&self, value: ColorControls) -> Result<(), Error> {
        if value.ally.is_some_and(|v| v > 0xffffff) || value.personal.is_some_and(|v| v > 0xffffff)
        {
            return Err(Error::InvalidPluginRequest(
                "Invalid ACT control colors".into(),
            ));
        }
        let flags = u32::from(value.ally.is_some()) | (u32::from(value.personal.is_some()) << 1);
        let mut packet = BytesMut::with_capacity(32);
        packet.put_u32_le(32);
        packet.put_u32_le(1);
        packet.put_u64_le(self.session);
        packet.put_u32_le(flags);
        packet.put_u32_le(value.ally.unwrap_or_default());
        packet.put_u32_le(value.personal.unwrap_or_default());
        packet.put_u32_le(1);
        self.request(Request::ColorSettings, &mut packet).await?;
        Ok(())
    }
    pub async fn set_color_scalars(&self, value: ColorScalars) -> Result<(), Error> {
        if !(-100..=100).contains(&value.luminosity) || !(-100..=100).contains(&value.saturation) {
            return Err(Error::InvalidPluginRequest(
                "Invalid ACT color offsets".into(),
            ));
        }
        let mut packet = BytesMut::with_capacity(32);
        packet.put_u32_le(32);
        packet.put_u32_le(1);
        packet.put_u64_le(self.session);
        packet.put_u32_le(u32::from(value.invert));
        packet.put_i32_le(i32::from(value.luminosity));
        packet.put_i32_le(i32::from(value.saturation));
        packet.put_u32_le(0);
        self.request(Request::ColorSettings, &mut packet).await?;
        Ok(())
    }
}
