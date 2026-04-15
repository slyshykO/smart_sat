#![no_std]
#![no_main]

extern crate libc;

use core::ffi::c_void;
use core::fmt::{self, Write};

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    unsafe { libc::abort() }
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_eh_personality() {}

struct FdWriter(libc::c_int);
impl Write for FdWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        unsafe {
            libc::write(self.0, s.as_ptr() as *const c_void, s.len());
        }
        Ok(())
    }
}

// ---- Linux SG (SCSI generic) UAPI (sg.h) ----
// SG_IO ioctl and sg_io_hdr layout are Linux UAPI. :contentReference[oaicite:1]{index=1}
const SG_IO: libc::Ioctl = 0x2285 as libc::Ioctl;
const SG_INTERFACE_ID_ORIG: libc::c_int = 'S' as libc::c_int;
const SG_DXFER_FROM_DEV: libc::c_int = -3;
const SG_INFO_OK_MASK: libc::c_uint = 0x1;
const SG_INFO_OK: libc::c_uint = 0x0;

#[repr(C)]
struct SgIoHdr {
    interface_id: libc::c_int,
    dxfer_direction: libc::c_int,
    cmd_len: libc::c_uchar,
    mx_sb_len: libc::c_uchar,
    iovec_count: libc::c_ushort,
    dxfer_len: libc::c_uint,
    dxferp: *mut c_void,
    cmdp: *mut libc::c_uchar,
    sbp: *mut libc::c_uchar,
    timeout: libc::c_uint,
    flags: libc::c_uint,
    pack_id: libc::c_int,
    usr_ptr: *mut c_void,
    status: libc::c_uchar,
    masked_status: libc::c_uchar,
    msg_status: libc::c_uchar,
    sb_len_wr: libc::c_uchar,
    host_status: libc::c_ushort,
    driver_status: libc::c_ushort,
    resid: libc::c_int,
    duration: libc::c_uint,
    info: libc::c_uint,
}

// ---- ATA SMART constants ----
// ATA_SMART_READ_VALUES=0xD0, SMART “password” LBAm=0x4F LBAh=0xC2. :contentReference[oaicite:2]{index=2}
const ATA_SMART_CMD: u8 = 0xB0;
const ATA_SMART_READ_VALUES: u8 = 0xD0;
const ATA_SMART_LBAM_PASS: u8 = 0x4F;
const ATA_SMART_LBAH_PASS: u8 = 0xC2;

// ---- Sense parsing (ATA Return Descriptor 0x09) ----
// smartmontools documents: des[3]=error, des[13]=status. :contentReference[oaicite:3]{index=3}
fn find_ata_return_descriptor(sense: &[u8]) -> Option<(u8 /*status*/, u8 /*error*/)> {
    if sense.len() < 8 {
        return None;
    }
    // Descriptor sense: bytes 8..(8+add_len)
    let add_len = sense[7] as usize;
    let end = core::cmp::min(sense.len(), 8 + add_len);
    let mut p = 8usize;
    while p + 2 <= end {
        let code = sense[p];
        let len = sense[p + 1] as usize;
        let next = p + 2 + len;
        if next > end {
            break;
        }
        if code == 0x09 && len >= 0x0c {
            // descriptor payload starts at p+2
            let payload = &sense[p + 2..p + 2 + len];
            let error = payload[1];   // des[3]
            let status = payload[11]; // des[13]
            return Some((status, error));
        }
        p = next;
    }
    None
}

// SMART attribute entry raw48 helper
fn raw48(le6: &[u8]) -> u64 {
    let mut v = 0u64;
    for (i, &b) in le6.iter().take(6).enumerate() {
        v |= (b as u64) << (8 * i);
    }
    v
}

fn decode_temperature_c(id: u8, raw6: &[u8]) -> Option<i16> {
    // Common ATA SMART temperature attributes. Encoding is vendor specific,
    // but often the low byte is current temperature in whole Celsius.
    if raw6.len() < 6 {
        return None;
    }
    if id != 190 && id != 194 && id != 231 {
        return None;
    }

    let primary = raw6[0] as i8 as i16;
    if (-40..=125).contains(&primary) {
        return Some(primary);
    }

    for &b in raw6.iter().take(6) {
        let t = b as i8 as i16;
        if (-40..=125).contains(&t) {
            return Some(t);
        }
    }
    None
}

#[unsafe(no_mangle)]
pub extern "C" fn main(argc: libc::c_int, argv: *const *const libc::c_char) -> libc::c_int {
    let mut out = FdWriter(1);
    let mut err = FdWriter(2);

    if argc < 2 {
        let _ = writeln!(err, "usage: smart_sat /dev/sdX");
        return 2;
    }

    let path = unsafe { *argv.add(1) };
    let fd = unsafe { libc::open(path, libc::O_RDWR | libc::O_CLOEXEC) };
    if fd < 0 {
        unsafe { libc::perror(b"open\0".as_ptr() as *const libc::c_char) };
        return 3;
    }

    let mut data = [0u8; 512];
    let mut sense = [0u8; 64];

    // Build ATA PASS-THROUGH(16) CDB (0x85).
    // Field packing matches SAT as shown in smartmontools. :contentReference[oaicite:4]{index=4}
    let mut cdb = [0u8; 16];
    cdb[0] = 0x85;
    // cdb[1] = (protocol<<1) | extend ; protocol=4 => PIO data-in
    cdb[1] = (4u8 << 1) | 0;
    // cdb[2] = (ck_cond<<5) | (t_dir<<3) | (byte_block<<2) | t_length
    // ck_cond=1 (ask for ATA Return Descriptor), t_dir=1 (from dev),
    // byte_block=1 (512B blocks), t_length=2 (sector_count holds count).
    cdb[2] = (1u8 << 5) | (1u8 << 3) | (1u8 << 2) | 2u8;

    // features (7:0) = SMART READ VALUES
    cdb[4] = ATA_SMART_READ_VALUES;
    // sector_count (7:0) = 1 sector (512 bytes)
    cdb[6] = 1;

    // LBAm/LBAh “SMART password”
    cdb[10] = ATA_SMART_LBAM_PASS;
    cdb[12] = ATA_SMART_LBAH_PASS;

    // command = SMART
    cdb[14] = ATA_SMART_CMD;

    let mut hdr = SgIoHdr {
        interface_id: SG_INTERFACE_ID_ORIG,
        dxfer_direction: SG_DXFER_FROM_DEV,
        cmd_len: 16,
        mx_sb_len: sense.len() as u8,
        iovec_count: 0,
        dxfer_len: data.len() as libc::c_uint,
        dxferp: data.as_mut_ptr() as *mut c_void,
        cmdp: cdb.as_mut_ptr(),
        sbp: sense.as_mut_ptr(),
        timeout: 20_000,
        flags: 0,
        pack_id: 0,
        usr_ptr: core::ptr::null_mut(),
        status: 0,
        masked_status: 0,
        msg_status: 0,
        sb_len_wr: 0,
        host_status: 0,
        driver_status: 0,
        resid: 0,
        duration: 0,
        info: 0,
    };

    let rc = unsafe { libc::ioctl(fd, SG_IO, &mut hdr as *mut _ as *mut c_void) };
    if rc < 0 {
        unsafe { libc::perror(b"ioctl(SG_IO)\0".as_ptr() as *const libc::c_char) };
        unsafe { libc::close(fd) };
        return 4;
    }

    // SG_INFO_OK means “no sense/driver noise”. :contentReference[oaicite:5]{index=5}
    if (hdr.info & SG_INFO_OK_MASK) != SG_INFO_OK {
        if let Some((st, er)) = find_ata_return_descriptor(&sense) {
            // ATA status bit0 (ERR) indicates error
            if (st & 0x01) != 0 {
                let _ = writeln!(err, "ATA error: status=0x{:02x} error=0x{:02x}", st, er);
                unsafe { libc::close(fd) };
                return 5;
            }
        } else {
            let _ = writeln!(err, "SG_IO reported abnormal status; no ATA Return Descriptor found");
            unsafe { libc::close(fd) };
            return 6;
        }
    }

    // Optional checksum sanity: sum of all 512 bytes should be 0 mod 256 (common SMART page convention)
    let mut sum: u8 = 0;
    for &b in data.iter() {
        sum = sum.wrapping_add(b);
    }
    if sum != 0 {
        let _ = writeln!(err, "warning: SMART page checksum mismatch (sum!=0)");
    }

    let _ = writeln!(out, "SMART attributes (selected):");
    let mut temperature_c: Option<i16> = None;
    // Attribute table typically: offset 2, 30 entries, 12 bytes each.
    for i in 0..30usize {
        let base = 2 + i * 12;
        let id = data[base];
        if id == 0 {
            continue;
        }
        let raw6 = &data[base + 5..base + 11];
        let raw = raw48(raw6);

        if temperature_c.is_none() {
            temperature_c = decode_temperature_c(id, raw6);
        }

        // A few common IDs (vendor-specific interpretation still applies):
        if id == 5 || id == 9 || id == 194 || id == 197 || id == 198 {
            let _ = writeln!(out, "  id {:3} raw {}", id, raw);
        }
    }

    match temperature_c {
        Some(t) => {
            let _ = writeln!(out, "disk temperature: {} C", t);
        }
        None => {
            let _ = writeln!(out, "disk temperature: unavailable (no temp attribute found)");
        }
    }

    unsafe { libc::close(fd) };
    0
}
