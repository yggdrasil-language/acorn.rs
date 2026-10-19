//! Raw Deflate stream decoder (RFC 1951).

use crate::{
    bit::{BitOrder, BitPosition, BitReader},
    error::CodecError,
    facts::{Canonicality, DecodeFacts},
    limits::CodecLimits,
};

use super::{
    huffman::{
        CODE_LENGTH_ORDER, DIST_BASE, DIST_EXTRA, HuffmanTable, LENGTH_BASE, LENGTH_EXTRA, fixed_distance_table,
        fixed_literal_table,
    },
    window::SlidingWindow,
};

/// Decodes a raw Deflate stream into owned bytes.
pub fn decode_raw(input: &[u8], limits: CodecLimits) -> Result<DecodeFacts<Vec<u8>>, CodecError> {
    let mut reader = BitReader::new(input, BitOrder::LeastSignificantBitFirst);
    let mut window = SlidingWindow::new();
    let mut output = Vec::new();
    let output_limit = limits.output_limit();
    let mut block_count = 0u64;
    let mut steps = 0u64;

    loop {
        if let Some(max) = limits.max_block_count {
            if block_count >= max {
                return Err(CodecError::OutputLimitExceeded { produced: output.len() as u64, limit: max });
            }
        }

        let bfinal = reader.read_bits(1)? != 0;
        let btype = reader.read_bits(2)?;

        match btype {
            0 => decode_stored_block(&mut reader, &mut window, &mut output, output_limit)?,
            1 => {
                let lit = fixed_literal_table();
                let dist = fixed_distance_table();
                decode_compressed_block(&mut reader, &lit, &dist, &mut window, &mut output, output_limit, &limits, &mut steps)?;
            }
            2 => {
                let (lit, dist) = decode_dynamic_tables(&mut reader, &limits)?;
                decode_compressed_block(&mut reader, &lit, &dist, &mut window, &mut output, output_limit, &limits, &mut steps)?;
            }
            _ => {
                return Err(CodecError::UnsupportedFeature { feature: crate::error::CodecFeature::BlockType(btype as u8) });
            }
        }

        block_count += 1;
        if !limits.check_expansion(input.len() as u64, output.len() as u64) {
            return Err(CodecError::ExpansionRatioExceeded {
                input: input.len() as u64,
                output: output.len() as u64,
                limit: limits.max_expansion_ratio.unwrap_or(crate::error::Ratio::new(1, 1)),
            });
        }
        if bfinal {
            break;
        }
    }

    let input_consumed = reader.consumed_bytes();
    let trailing = input.len() - input_consumed;

    let output_produced = output.len();
    Ok(DecodeFacts {
        value: output,
        input_consumed,
        output_produced,
        complete: true,
        trailing_input: trailing,
        canonical: Canonicality::NotApplicable,
    })
}

fn check_output_limit(output_len: usize, limit: u64) -> Result<(), CodecError> {
    if output_len as u64 >= limit {
        Err(CodecError::OutputLimitExceeded { produced: output_len as u64, limit })
    }
    else {
        Ok(())
    }
}

fn decode_stored_block(
    reader: &mut BitReader<'_>,
    window: &mut SlidingWindow,
    output: &mut Vec<u8>,
    output_limit: u64,
) -> Result<(), CodecError> {
    reader.align_to_byte();
    let len = reader.read_u16_le()?;
    let nlen = reader.read_u16_le()?;
    if len as u16 != (nlen ^ 0xFFFF) {
        return Err(CodecError::InvalidBlockHeader { position: reader.position() });
    }
    for _ in 0..len {
        let byte = reader.read_byte()?;
        check_output_limit(output.len(), output_limit)?;
        window.push_literal(byte, output);
    }
    Ok(())
}

fn decode_dynamic_tables(reader: &mut BitReader<'_>, limits: &CodecLimits) -> Result<(HuffmanTable, HuffmanTable), CodecError> {
    let hlit = reader.read_bits(5)? as usize + 257;
    let hdist = reader.read_bits(5)? as usize + 1;
    let hclen = reader.read_bits(4)? as usize + 4;

    if let Some(max_symbols) = limits.max_huffman_symbols {
        if (hlit + hdist) as u64 > max_symbols {
            return Err(CodecError::InvalidHuffmanTree { position: reader.position() });
        }
    }

    let mut code_lengths = vec![0u8; 19];
    for index in 0..hclen {
        code_lengths[CODE_LENGTH_ORDER[index] as usize] = reader.read_bits(3)? as u8;
    }
    let code_table = build_huffman(&code_lengths, reader.position())?;

    let mut lengths = vec![0u8; hlit + hdist];
    let mut index = 0usize;
    while index < lengths.len() {
        let symbol = code_table.decode(reader)?;
        match symbol {
            0..=15 => {
                lengths[index] = symbol as u8;
                index += 1;
            }
            16 => {
                let repeat = reader.read_bits(2)? as usize + 3;
                if index == 0 || index + repeat > lengths.len() {
                    return Err(CodecError::InvalidHuffmanTree { position: reader.position() });
                }
                let prev = lengths[index - 1];
                lengths[index..index + repeat].fill(prev);
                index += repeat;
            }
            17 => {
                let repeat = reader.read_bits(3)? as usize + 3;
                if index + repeat > lengths.len() {
                    return Err(CodecError::InvalidHuffmanTree { position: reader.position() });
                }
                index += repeat;
            }
            18 => {
                let repeat = reader.read_bits(7)? as usize + 11;
                if index + repeat > lengths.len() {
                    return Err(CodecError::InvalidHuffmanTree { position: reader.position() });
                }
                index += repeat;
            }
            _ => {
                return Err(CodecError::HuffmanSymbolOutOfRange { position: reader.position(), symbol });
            }
        }
    }

    let position = reader.position();
    let lit = build_huffman(&lengths[..hlit], position)?;
    let dist = if lengths[hlit..].iter().all(|&len| len == 0) {
        HuffmanTable::empty()
    }
    else {
        build_huffman(&lengths[hlit..], position)?
    };
    Ok((lit, dist))
}

fn build_huffman(lengths: &[u8], position: BitPosition) -> Result<HuffmanTable, CodecError> {
    HuffmanTable::from_lengths(lengths).map_err(|_| CodecError::InvalidHuffmanTree { position })
}

fn decode_compressed_block(
    reader: &mut BitReader<'_>,
    lit_table: &HuffmanTable,
    dist_table: &HuffmanTable,
    window: &mut SlidingWindow,
    output: &mut Vec<u8>,
    output_limit: u64,
    limits: &CodecLimits,
    steps: &mut u64,
) -> Result<(), CodecError> {
    loop {
        if let Some(max_steps) = limits.max_steps {
            if *steps >= max_steps {
                return Err(CodecError::OutputLimitExceeded { produced: output.len() as u64, limit: max_steps });
            }
        }
        *steps += 1;

        let symbol = lit_table.decode(reader)?;
        if symbol < 256 {
            check_output_limit(output.len(), output_limit)?;
            window.push_literal(symbol as u8, output);
        }
        else if symbol == 256 {
            break;
        }
        else {
            let length_index = (symbol - 257) as usize;
            if length_index >= LENGTH_BASE.len() {
                return Err(CodecError::HuffmanSymbolOutOfRange { position: reader.position(), symbol });
            }
            let extra_bits = LENGTH_EXTRA[length_index];
            let length = LENGTH_BASE[length_index] as u32 + if extra_bits > 0 { reader.read_bits(extra_bits)? } else { 0 };

            let dist_symbol = dist_table.decode(reader)?;
            let dist_index = dist_symbol as usize;
            if dist_index >= DIST_BASE.len() {
                return Err(CodecError::HuffmanSymbolOutOfRange { position: reader.position(), symbol: dist_symbol });
            }
            let dist_extra = DIST_EXTRA[dist_index];
            let distance = DIST_BASE[dist_index] as u32 + if dist_extra > 0 { reader.read_bits(dist_extra)? } else { 0 };

            if let Some(max_dist) = limits.max_distance {
                if distance > max_dist {
                    return Err(CodecError::DistanceOutOfWindow { position: reader.position(), distance, window: max_dist });
                }
            }

            let produced = output.len() as u64 + u64::from(length);
            if produced > output_limit {
                return Err(CodecError::OutputLimitExceeded { produced, limit: output_limit });
            }
            if !window.can_reference(distance) {
                return Err(CodecError::InvalidBackReference {
                    position: reader.position(),
                    distance,
                    available: window.len() as u32,
                });
            }
            window.copy_back(distance, length, output);
        }
    }
    Ok(())
}
