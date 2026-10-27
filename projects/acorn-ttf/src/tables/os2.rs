//! `OS/2` — legacy metric information.
//!
//! This parser understands versions 0..=5, all of which overlap nicely.

use crate::reader::{ReadError, Reader};

/// OS/2 table — covers versions 0 through 5.
#[derive(Debug, Clone, PartialEq)]
pub struct Os2Table {
    /// Version 0..5.
    pub version: u16,
    /// `xAvgCharWidth`.
    pub x_avg_char_width: i16,
    /// `usWeightClass`.
    pub us_weight_class: u16,
    /// `usWidthClass`.
    pub us_width_class: u16,
    /// `fsType`.
    pub fs_type: u16,
    /// `ySubscriptXSize`.
    pub y_subscript_x_size: i16,
    /// `ySubscriptYSize`.
    pub y_subscript_y_size: i16,
    /// `ySubscriptXOffset`.
    pub y_subscript_x_offset: i16,
    /// `ySubscriptYOffset`.
    pub y_subscript_y_offset: i16,
    /// `ySuperscriptXSize`.
    pub y_superscript_x_size: i16,
    /// `ySuperscriptYSize`.
    pub y_superscript_y_size: i16,
    /// `ySuperscriptXOffset`.
    pub y_superscript_x_offset: i16,
    /// `ySuperscriptYOffset`.
    pub y_superscript_y_offset: i16,
    /// `yStrikeoutSize`.
    pub y_strikeout_size: i16,
    /// `yStrikeoutPosition`.
    pub y_strikeout_position: i16,
    /// `sFamilyClass`.
    pub s_family_class: i16,
    /// `panose[10]`.
    pub panose: [u8; 10],
    /// `ulUnicodeRange[4]`.
    pub ul_unicode_range: [u32; 4],
    /// `achVendID` (tag).
    pub ach_vend_id: crate::reader::Tag,
    /// `fsSelection`.
    pub fs_selection: u16,
    /// `usFirstCharIndex`.
    pub us_first_char_index: u16,
    /// `usLastCharIndex`.
    pub us_last_char_index: u16,

    // Version 1 additions
    /// `sTypoAscender`.
    pub s_typo_ascender: Option<i16>,
    /// `sTypoDescender`.
    pub s_typo_descender: Option<i16>,
    /// `sTypoLineGap`.
    pub s_typo_line_gap: Option<i16>,
    /// `usWinAscent`.
    pub us_win_ascent: Option<u16>,
    /// `usWinDescent`.
    pub us_win_descent: Option<u16>,

    // Version 2 additions
    /// `ulCodePageRange[2]`.
    pub ul_code_page_range: Option<[u32; 2]>,

    // Version 4 additions
    /// `sxHeight`.
    pub sx_height: Option<i16>,
    /// `sCapHeight`.
    pub s_cap_height: Option<i16>,
    /// `usDefaultChar`.
    pub us_default_char: Option<u16>,
    /// `usBreakChar`.
    pub us_break_char: Option<u16>,
    /// `usMaxContext`.
    pub us_max_context: Option<u16>,
}

impl Os2Table {
    pub fn parse(r: &mut Reader<'_>) -> Result<Self, ReadError> {
        let version = r.read_u16()?;
        let x_avg_char_width = r.read_i16()?;
        let us_weight_class = r.read_u16()?;
        let us_width_class = r.read_u16()?;
        let fs_type = r.read_u16()?;
        let y_subscript_x_size = r.read_i16()?;
        let y_subscript_y_size = r.read_i16()?;
        let y_subscript_x_offset = r.read_i16()?;
        let y_subscript_y_offset = r.read_i16()?;
        let y_superscript_x_size = r.read_i16()?;
        let y_superscript_y_size = r.read_i16()?;
        let y_superscript_x_offset = r.read_i16()?;
        let y_superscript_y_offset = r.read_i16()?;
        let y_strikeout_size = r.read_i16()?;
        let y_strikeout_position = r.read_i16()?;
        let s_family_class = r.read_i16()?;
        let panose = [
            r.read_u8()?,
            r.read_u8()?,
            r.read_u8()?,
            r.read_u8()?,
            r.read_u8()?,
            r.read_u8()?,
            r.read_u8()?,
            r.read_u8()?,
            r.read_u8()?,
            r.read_u8()?,
        ];
        let ul_unicode_range = [r.read_u32()?, r.read_u32()?, r.read_u32()?, r.read_u32()?];
        let ach_vend_id = r.read_tag()?;
        let fs_selection = r.read_u16()?;
        let us_first_char_index = r.read_u16()?;
        let us_last_char_index = r.read_u16()?;

        let (s_typo_ascender, s_typo_descender, s_typo_line_gap, us_win_ascent, us_win_descent) = if version >= 1 {
            (Some(r.read_i16()?), Some(r.read_i16()?), Some(r.read_i16()?), Some(r.read_u16()?), Some(r.read_u16()?))
        }
        else {
            (None, None, None, None, None)
        };

        let ul_code_page_range = if version >= 2 { Some([r.read_u32()?, r.read_u32()?]) } else { None };

        // Version 3 added subscript/superscript sizes again? No, those are in version 2 too — skip any reserved.
        if version == 3 {
            // Skip 10 reserved bytes (sTypoAscender etc already present in v1).
            let _ = r.skip(10);
        }

        let (sx_height, s_cap_height, us_default_char, us_break_char, us_max_context) = if version >= 4 {
            (Some(r.read_i16()?), Some(r.read_i16()?), Some(r.read_u16()?), Some(r.read_u16()?), Some(r.read_u16()?))
        }
        else {
            (None, None, None, None, None)
        };

        Ok(Self {
            version,
            x_avg_char_width,
            us_weight_class,
            us_width_class,
            fs_type,
            y_subscript_x_size,
            y_subscript_y_size,
            y_subscript_x_offset,
            y_subscript_y_offset,
            y_superscript_x_size,
            y_superscript_y_size,
            y_superscript_x_offset,
            y_superscript_y_offset,
            y_strikeout_size,
            y_strikeout_position,
            s_family_class,
            panose,
            ul_unicode_range,
            ach_vend_id,
            fs_selection,
            us_first_char_index,
            us_last_char_index,
            s_typo_ascender,
            s_typo_descender,
            s_typo_line_gap,
            us_win_ascent,
            us_win_descent,
            ul_code_page_range,
            sx_height,
            s_cap_height,
            us_default_char,
            us_break_char,
            us_max_context,
        })
    }
}
