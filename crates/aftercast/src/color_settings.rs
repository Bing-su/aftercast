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
/// Current worker values. Colors retain the complete ARGB value; None is Color.Empty.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RuntimeColors {
    pub invert: bool,
    pub luminosity: f32,
    pub saturation: f32,
    pub ally: Option<u32>,
    pub personal: Option<u32>,
}
impl HostHandle {
    /// Read once from the worker owner; this never changes settings or publishes events.
    pub async fn runtime_colors(&self) -> Result<RuntimeColors, Error> {
        let mut packet = BytesMut::with_capacity(48);
        packet.put_u32_le(48);
        packet.put_u32_le(1);
        packet.put_u64_le(self.session);
        packet.put_u32_le(0);
        packet.put_u32_le(0);
        packet.put_u32_le(0);
        packet.put_u32_le(2);
        packet.put_u64_le(0);
        packet.put_u64_le(0);
        self.request(Request::ColorSettings, &mut packet).await?;
        decode_runtime_colors(&packet, self.session)
    }
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

fn decode_runtime_colors(packet: &[u8], session: u64) -> Result<RuntimeColors, Error> {
    fn word(packet: &[u8], offset: usize) -> Option<u32> {
        Some(u32::from_le_bytes(
            packet.get(offset..offset + 4)?.try_into().ok()?,
        ))
    }
    let invalid = || Error::InvalidPluginRequest("Invalid color snapshot response".into());
    if packet.len() != 48
        || word(packet, 0) != Some(48)
        || word(packet, 4) != Some(1)
        || packet.get(8..16) != Some(session.to_le_bytes().as_slice())
        || word(packet, 28) != Some(2)
        || word(packet, 44) != Some(0)
    {
        return Err(invalid());
    }
    let invert = match word(packet, 16) {
        Some(0) => false,
        Some(1) => true,
        _ => return Err(invalid()),
    };
    let luminosity = f32::from_bits(word(packet, 20).ok_or_else(invalid)?);
    let saturation = f32::from_bits(word(packet, 24).ok_or_else(invalid)?);
    let flags = word(packet, 32).ok_or_else(invalid)?;
    let ally = word(packet, 36).ok_or_else(invalid)?;
    let personal = word(packet, 40).ok_or_else(invalid)?;
    if !luminosity.is_finite()
        || !saturation.is_finite()
        || !(-1.0..=1.0).contains(&luminosity)
        || !(-1.0..=1.0).contains(&saturation)
        || flags > 3
        || flags & 1 == 0 && ally != 0
        || flags & 2 == 0 && personal != 0
    {
        return Err(invalid());
    }
    Ok(RuntimeColors {
        invert,
        luminosity,
        saturation,
        ally: (flags & 1 != 0).then_some(ally),
        personal: (flags & 2 != 0).then_some(personal),
    })
}

#[cfg(test)]
mod snapshot_tests {
    use super::*;
    #[test]
    fn reject_malformed_or_wrong_session_snapshots() {
        let mut data = [0_u8; 48];
        data[0..4].copy_from_slice(&48_u32.to_le_bytes());
        data[4..8].copy_from_slice(&1_u32.to_le_bytes());
        data[8..16].copy_from_slice(&7_u64.to_le_bytes());
        data[28..32].copy_from_slice(&2_u32.to_le_bytes());
        decode_runtime_colors(&data, 7).expect("valid snapshot");
        decode_runtime_colors(&data, 8).expect_err("wrong session");
        data[20..24].copy_from_slice(&f32::NAN.to_bits().to_le_bytes());
        decode_runtime_colors(&data, 7).expect_err("invalid snapshot");
        data[20..24].copy_from_slice(&0_u32.to_le_bytes());
        data[36..40].copy_from_slice(&1_u32.to_le_bytes());
        decode_runtime_colors(&data, 7).expect_err("invalid snapshot");
        data[32..36].copy_from_slice(&1_u32.to_le_bytes());
        assert_eq!(decode_runtime_colors(&data, 7).unwrap().ally, Some(1));
    }
}
