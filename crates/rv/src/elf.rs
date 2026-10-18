//! Minimal loader for statically linked 32-bit little-endian RISC-V ELF
//! executables: copies every PT_LOAD segment into a memory image.

use makine_lang::tr;

#[derive(Debug, PartialEq, Eq)]
pub struct Image {
    pub entry: u32,
    /// (address, bytes) of each loadable segment, zero-filled to memsz.
    pub segments: Vec<(u32, Vec<u8>)>,
}

impl Image {
    /// Highest address used plus one.
    pub fn end(&self) -> u32 {
        self.segments
            .iter()
            .map(|(a, b)| a + b.len() as u32)
            .max()
            .unwrap_or(0)
    }
}

pub fn load(data: &[u8]) -> Result<Image, String> {
    let u16_at = |o: usize| -> Result<u16, String> {
        Ok(u16::from_le_bytes(
            data.get(o..o + 2)
                .ok_or(tr!("kesik ELF", "truncated ELF"))?
                .try_into()
                .unwrap(),
        ))
    };
    let u32_at = |o: usize| -> Result<u32, String> {
        Ok(u32::from_le_bytes(
            data.get(o..o + 4)
                .ok_or(tr!("kesik ELF", "truncated ELF"))?
                .try_into()
                .unwrap(),
        ))
    };
    if data.get(..4) != Some(b"\x7fELF") {
        return Err(tr!("ELF dosyası değil", "not an ELF file").into());
    }
    if data[4] != 1 || data[5] != 1 {
        return Err(tr!(
            "32-bit little-endian ELF bekleniyordu",
            "expected a 32-bit little-endian ELF"
        )
        .into());
    }
    if u16_at(18)? != 243 {
        return Err(tr!("RISC-V ELF değil", "not a RISC-V ELF").into());
    }
    let entry = u32_at(24)?;
    let phoff = u32_at(28)? as usize;
    let phentsize = u16_at(42)? as usize;
    let phnum = u16_at(44)? as usize;
    let mut segments = Vec::new();
    for i in 0..phnum {
        let h = phoff + i * phentsize;
        if u32_at(h)? != 1 {
            continue; // not PT_LOAD
        }
        let offset = u32_at(h + 4)? as usize;
        let paddr = u32_at(h + 12)?;
        let filesz = u32_at(h + 16)? as usize;
        let memsz = u32_at(h + 20)? as usize;
        let mut bytes = data
            .get(offset..offset + filesz)
            .ok_or(tr!("kesik segment", "truncated segment"))?
            .to_vec();
        bytes.resize(memsz, 0);
        segments.push((paddr, bytes));
    }
    Ok(Image { entry, segments })
}
