use std::borrow::Cow;

use ascii::AsAsciiStr;

#[derive(thiserror::Error, Debug, Clone, Copy)]
pub enum AnselErr {
    #[error("the byte at index {offset} (value 0x{value:x}) is not ANSEL")]
    Invalid { offset: usize, value: u8 },

    #[error("stacked combining characters are not allowed")]
    StackedCombiningChars { offset: usize },

    #[error("combining character at end of input")]
    CombiningCharacterAtEnd { offset: usize },
}

impl AnselErr {
    pub fn offset(self) -> usize {
        match self {
            AnselErr::Invalid { offset, .. } => offset,
            AnselErr::StackedCombiningChars { offset } => offset,
            AnselErr::CombiningCharacterAtEnd { offset } => offset,
        }
    }
}

pub fn decode(input: &[u8]) -> Result<Cow<str>, AnselErr> {
    match input.as_ascii_str() {
        // if it’s pure ASCII we don’t need to do anything
        Ok(ascii_str) => Ok(Cow::Borrowed(ascii_str.as_str())),
        Err(ascii_err) => {
            let mut dest = String::new();
            let mut after_next = None;

            let mut input = input;
            let mut ascii_err = ascii_err;

            loop {
                let mut valid_part =
                    unsafe { input[0..ascii_err.valid_up_to()].as_ascii_str_unchecked() };

                if !valid_part.is_empty() {
                    if let Some(after_next) = after_next.take() {
                        dest.push(valid_part[0].as_char());
                        dest.push(after_next);

                        valid_part = &valid_part[1..];
                    }
                }

                dest.push_str(valid_part.as_str());

                let input_c = input[ascii_err.valid_up_to()];
                input = &input[ascii_err.valid_up_to() + 1..];

                // combining chars
                if matches!(input_c, b'\xE0'..=b'\xFB' | b'\xFE') {
                    let combining = match input_c {
                        b'\xE0' => '\u{0309}',
                        b'\xE1' => '\u{0300}',
                        b'\xE2' => '\u{0301}',
                        b'\xE3' => '\u{0302}',
                        b'\xE4' => '\u{0303}',
                        b'\xE5' => '\u{0304}',
                        b'\xE6' => '\u{0306}',
                        b'\xE7' => '\u{0307}',
                        b'\xE8' => '\u{0308}',
                        b'\xE9' => '\u{030C}',
                        b'\xEA' => '\u{030A}',
                        b'\xEB' => '\u{FE20}',
                        b'\xEC' => '\u{FE20}',
                        b'\xED' => '\u{0315}',
                        b'\xEE' => '\u{030B}',
                        b'\xEF' => '\u{0310}',
                        b'\xF0' => '\u{0327}',
                        b'\xF1' => '\u{0328}',
                        b'\xF2' => '\u{0323}',
                        b'\xF3' => '\u{0324}',
                        b'\xF4' => '\u{0325}',
                        b'\xF5' => '\u{0333}',
                        b'\xF6' => '\u{0332}',
                        b'\xF7' => '\u{0326}',
                        b'\xF8' => '\u{031C}',
                        b'\xF9' => '\u{032E}',
                        b'\xFA' => '\u{FE22}',
                        b'\xFB' => '\u{FE23}',
                        b'\xFE' => '\u{0313}',
                        _ => unreachable!(),
                    };

                    if let Some(_after_next) = after_next.take() {
                        return Err(AnselErr::StackedCombiningChars {
                            offset: ascii_err.valid_up_to(),
                        });
                    }

                    after_next = Some(combining);
                } else {
                    let output_c = match input_c {
                        // ANSI/NISO Z39.47-1993 (R2003)
                        // Ax
                        b'\xA1' => '\u{0141}',
                        b'\xA2' => '\u{00D8}',
                        b'\xA3' => '\u{0110}',
                        b'\xA4' => '\u{00DE}',
                        b'\xA5' => '\u{00C6}',
                        b'\xA6' => '\u{0152}',
                        b'\xA7' => '\u{02B9}',
                        b'\xA8' => '\u{00B7}',
                        b'\xA9' => '\u{266D}',
                        b'\xAA' => '\u{00AE}',
                        b'\xAB' => '\u{00B1}',
                        b'\xAC' => '\u{01A0}',
                        b'\xAD' => '\u{01AF}',
                        b'\xAE' => '\u{02BC}',
                        // Bx
                        b'\xB0' => '\u{02BB}',
                        b'\xB1' => '\u{0142}',
                        b'\xB2' => '\u{00F8}',
                        b'\xB3' => '\u{0111}',
                        b'\xB4' => '\u{00FE}',
                        b'\xB5' => '\u{00E6}',
                        b'\xB6' => '\u{0153}',
                        b'\xB7' => '\u{02BA}',
                        b'\xB8' => '\u{0131}',
                        b'\xB9' => '\u{00A3}',
                        b'\xBA' => '\u{00F0}',
                        b'\xBC' => '\u{01A1}',
                        b'\xBD' => '\u{01B0}',
                        // Cx
                        b'\xC0' => '\u{00B0}',
                        b'\xC1' => '\u{2113}',
                        b'\xC2' => '\u{2117}',
                        b'\xC3' => '\u{00A9}',
                        b'\xC4' => '\u{266F}',
                        b'\xC5' => '\u{00BF}',
                        b'\xC6' => '\u{00A1}',
                        // GEDCOM
                        b'\xBE' => '\u{25A1}',
                        b'\xBF' => '\u{25A0}',
                        b'\xCD' => '\u{0065}',
                        b'\xCE' => '\u{006F}',
                        b'\xCF' => '\u{00DF}',
                        b'\xFC' => '\u{0338}',
                        // TODO: MARC21?
                        c => {
                            return Err(AnselErr::Invalid {
                                value: c,
                                offset: ascii_err.valid_up_to(),
                            })
                        }
                    };

                    dest.push(output_c);

                    if let Some(after_next) = after_next.take() {
                        dest.push(after_next);
                    }
                }

                ascii_err = match input.as_ascii_str() {
                    Ok(mut ascii_str) => {
                        // whole remainder (which might be empty) is valid ASCII
                        // still need to insert any combining characters
                        if ascii_str.is_empty() {
                            if after_next.is_some() {
                                return Err(AnselErr::CombiningCharacterAtEnd {
                                    offset: ascii_err.valid_up_to(),
                                });
                            }
                        } else {
                            if let Some(after_next) = after_next.take() {
                                dest.push(ascii_str[0].as_char());
                                dest.push(after_next);

                                ascii_str = &ascii_str[1..];
                            }

                            dest.push_str(ascii_str.as_str());
                        }

                        return Ok(Cow::Owned(dest));
                    }
                    Err(ascii_err) => ascii_err,
                };
            }
        }
    }
}
