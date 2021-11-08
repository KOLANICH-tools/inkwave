use std::collections::HashMap;
use std::*;

use io::IOBase;

use icecream::ic;
use itertools::takewhile;
use pathlib::Path;
use plumbum::cli;
use zlib::crc32;

const MAX_WAVEFORMS: u16 = 4096;
const MAX_MODES: u16 = 256;
const MAX_TEMP_RANGES: u16 = 256;
const MYSTERIOUS_OFFSET: u16 = 63;

enum MODE {
	INIT = 0,
	DU = 1,
	GC16 = 2,
	GC16_FAST = 3,
	A2 = 4,
	GL16 = 5,
	GL16_FAST = 6,
	DU4 = 7,
	REAGL = 8,
	REAGLD = 9,
	GL4 = 10,
	GL16_INV = 11,
}
const update_modes: _ = [
	(
		MODE::INIT,
		"INIT (panel initialization / clear screen to white)",
	),
	(
		MODE::DU,
		"DU (direct update, gray to black/white transition, 1bpp)",
	),
	(MODE::GC16, "GC16 (high fidelity, flashing, 4bpp)"),
	(MODE::GC16_FAST, "GC16_FAST (medium fidelity, 4bpp)"),
	(
		MODE::A2,
		"A2 (animation update, fastest and lowest fidelity)",
	),
	(
		MODE::GL16,
		"GL16 (high fidelity from white transition, 4bpp)",
	),
	(
		MODE::GL16_FAST,
		"GL16_FAST (medium fidelity from white transition, 4bpp)",
	),
	(
		MODE::DU4,
		"DU4 (direct update, medium fidelity, text to text, 2bpp)",
	),
	(MODE::REAGL, "REAGL (non-flashing, ghost-compensation)"),
	(
		MODE::REAGLD,
		"REAGLD (non-flashing, ghost-compensation with dithering)",
	),
	(MODE::GL4, "GL4 (2-bit from white transition, 2bpp)"),
	(
		MODE::GL16_INV,
		"GL16_INV (high fidelity for black transition, 4bpp)",
	),
]
.iter()
.cloned()
.collect::<HashMap<_, _>>();
const mfg_codes: _ = [
	(51, "ED060SCF (V220 6\" Tequila)"),
	(52, "ED060SCFH1 (V220 Tequila Hydis – Line 2)"),
	(53, "ED060SCFH1 (V220 Tequila Hydis – Line 3)"),
	(54, "ED060SCFC1 (V220 Tequila CMO)"),
	(55, "ED060SCFT1 (V220 Tequila CPT)"),
	(56, "ED060SCG (V220 Whitney)"),
	(57, "ED060SCGH1 (V220 Whitney Hydis – Line 2)"),
	(58, "ED060SCGH1 (V220 Whitney Hydis – Line 3)"),
	(59, "ED060SCGC1 (V220 Whitney CMO)"),
	(60, "ED060SCGT1 (V220 Whitney CPT)"),
	(160, "Unknown LGD panel"),
	(161, "Unknown LGD panel"),
	(162, "Unknown LGD panel"),
	(163, "LB060S03-RD02 (LGD Tequila Line 1)"),
	(164, "2nd LGD Tequila Line"),
	(165, "LB060S05-RD02 (LGD Whitney Line 1)"),
	(166, "2nd LGD Whitney Line"),
	(167, "Unknown LGD panel"),
	(168, "Unknown LGD panel"),
	(202, "reMarkable panel?"),
]
.iter()
.cloned()
.collect::<HashMap<_, _>>();
const run_types: _ = [
	(0, "[B]aseline"),
	(1, "[T]est/trial"),
	(2, "[P]roduction"),
	(3, "[Q]ualification"),
	(4, "V110[A]"),
	(5, "V220[C]"),
	(6, "D"),
	(7, "V220[E]"),
	(8, "F"),
	(9, "G"),
	(10, "H"),
	(11, "I"),
	(12, "J"),
	(13, "K"),
	(14, "L"),
	(15, "M"),
	(16, "N"),
]
.iter()
.cloned()
.collect::<HashMap<_, _>>();
const fpl_platforms: _ = [
	(0, "Matrix 2.0"),
	(1, "Matrix 2.1"),
	(2, "Matrix 2.3 / Matrix Vixplex (V100)"),
	(3, "Matrix Vizplex 110 (V110)"),
	(4, "Matrix Vizplex 110A (V110A)"),
	(5, "Matrix Vizplex unknown"),
	(6, "Matrix Vizplex 220 (V220)"),
	(7, "Matrix Vizplex 250 (V250)"),
	(8, "Matrix Vizplex 220E (V220E)"),
]
.iter()
.cloned()
.collect::<HashMap<_, _>>();
const fpl_sizes: _ = [
	(0, "5.0\""),
	(1, "6.0\""),
	(2, "6.1\""),
	(3, "6.3\""),
	(4, "8.0\""),
	(5, "9.7\""),
	(6, "9.9\""),
	(7, "Unknown"),
	(50, "5\", unknown resolution"),
	(60, "6\", 800x600"),
	(61, "6.1\", 1024x768"),
	(63, "6\", 800x600"),
	(80, "8\", unknown resolution"),
	(97, "9.7\", 1200x825"),
	(99, "9.7\", 1600x1200"),
]
.iter()
.cloned()
.collect::<HashMap<_, _>>();
const fpl_rates: _ = [(80, "50Hz"), (96, "60Hz"), (133, "85Hz")]
	.iter()
	.cloned()
	.collect::<HashMap<_, _>>();
const mode_versions: _ = [
	(0, "MU/GU/GC/PU (V100 modes)"),
	(1, "DU/GC16/GC4 (V110/V110A modes)"),
	(2, "DU/GC16/GC4 (V110/V110A modes)"),
	(3, "DU/GC16/GC4/AU (V220, 50Hz/85Hz modes)"),
	(4, "DU/GC16/AU (V220, 85Hz modes)"),
	(6, "? (V220: 210 dpi: 85Hz modes)"),
	(7, "? (V220, 210 dpi, 85Hz modes)"),
]
.iter()
.cloned()
.collect::<HashMap<_, _>>();
const waveform_types: _ = [
	(0, "WX"),
	(1, "WY"),
	(2, "WP"),
	(3, "WZ"),
	(4, "WQ"),
	(5, "TA"),
	(6, "WU"),
	(7, "TB"),
	(8, "TD"),
	(9, "WV"),
	(10, "WT"),
	(11, "TE"),
	(12, "XA"),
	(13, "XB"),
	(14, "WE"),
	(15, "WD"),
	(16, "XC"),
	(17, "VE"),
	(18, "XD"),
	(19, "XE"),
	(20, "XF"),
	(21, "WJ"),
	(22, "WK"),
	(23, "WL"),
	(24, "VJ"),
	(43, "WR"),
	(60, "AA"),
	(75, "AC"),
	(76, "BD"),
	(80, "AE"),
]
.iter()
.cloned()
.collect::<HashMap<_, _>>();
const waveform_tuning_biases: _ = [
	(0, "Standard"),
	(1, "Increased DS Blooming V110/V110E"),
	(2, "Increased DS Blooming V220/V220E"),
	(3, "Improved temperature range"),
	(4, "GC16 fast"),
	(5, "GC16 fast, GL16 fast"),
	(6, "Unknown"),
]
.iter()
.cloned()
.collect::<HashMap<_, _>>();
fn get_desc(table: Mapping<i32, &str>, key: i32, default: &str) -> &str {
	if table.iter().any(|&x| x == key) {
		return table[key];
	}
	if default {
		return default;
	}
	return "Unknown";
}
fn pri32_modes(mode_count: u8) {
	let i: u8 = 0;
	let desc: str = "";
	println!("{:?} ", "Modes in file:");
	for i in (0..mode_count) {
		i = MODE(i);
		let mut desc = get_desc(update_modes, i, "Unknown mode");
		println!("{:?} ", "\t{:2d}: {}".format(i, desc));
	}
	println!("{:?} ", "");
}
fn get_desc_mfg_code(mfg_code: u32) -> &str {
	let desc: str = get_desc(mfg_codes, mfg_code, "");
	if desc {
		return desc;
	}
	if mfg_code >= 51 && mfg_code < 60 {
		return "PVI/EIH panel ";
	}
	if mfg_code >= 160 && mfg_code < 168 {
		return "LGD panel ";
	}
	return "Unknown code ";
}
struct waveform_data_header {
	xwia: ST0,
	wmta: ST1,
}

/*
"
	struct waveform_data_header {
			u32 checksum; # 0
			u32 filesize; # 4
			u32 serial; # 8 serial number
			u8 run_type; # 12
			u8 fpl_platform; # 13
			u16 fpl_lot; # 14
			u8 mode_version_or_adhesive_run_num; # 16
			u8 waveform_version; # 17
			u8 waveform_subversion; # 18
			u8 waveform_type; # 19
			u8 fpl_size; # 20 (aka panel_size)
			u8 mfg_code; # 21 (aka amepd_part_number)
			u8 waveform_tuning_bias_or_rev; # 22
			u8 fpl_rate; # 23 (aka frame_rate)
			u8 unknown0; # 24
			u8 vcom_shifted; # 25
			u16 unknown1; # 26
			u16 xwia_LO; # 28 # address of extra waveform information
			u8 xwia_HI; # 30 address of extra waveform information
			u8 cs1; # 31 checksum 1
			u16 wmta_LO # 32;
			u8 wmta_HI # 34;
			u8 fvsn;
			u8 luts;
			u8 mc; # mode count (length of mode table - 1)
			u8 trc; # temperature range count (length of temperature table - 1)
			u8 advanced_wfm_flags;
			u8 eb;
			u8 sb;
			u8 reserved0_1;
			u8 reserved0_2;
			u8 reserved0_3;
			u8 reserved0_4;
			u8 reserved0_5;
			u8 cs2; # checksum 2
	}__attribute__((packed));";
*/
impl waveform_data_header {
	const __slots__: _ = (
		"checksum",
		"filesize",
		"serial",
		"run_type",
		"fpl_platform",
		"fpl_lot",
		"mode_version_or_adhesive_run_num",
		"waveform_version",
		"waveform_subversion",
		"waveform_type",
		"fpl_size",
		"mfg_code",
		"waveform_tuning_bias_or_rev",
		"fpl_rate",
		"unknown0",
		"vcom_shifted",
		"unknown1",
		"xwia",
		"cs1",
		"wmta",
		"fvsn",
		"luts",
		"mc",
		"trc",
		"advanced_wfm_flags",
		"eb",
		"sb",
		"reserved0_1",
		"reserved0_2",
		"reserved0_3",
		"reserved0_4",
		"reserved0_5",
		"cs2",
	);
	const structStr: _ = "IIIBBHBBBBBBBBBBHHBBHBBBBBBBBBBBBBB";
	const parser: _ = struct_.Struct(structStr);
	const structSize: _ = struct_.calcsize(structStr);
	fn __init__(&self, data: &[u8]) {
		let [checksum, filesize, serial, run_type, fpl_platform, fpl_lot, mode_version_or_adhesive_run_num, waveform_version, waveform_subversion, waveform_type, fpl_size, mfg_code, waveform_tuning_bias_or_rev, fpl_rate, unknown0, vcom_shifted, unknown1, xwia_LO, xwia_HI, cs1, wmta_LO, wmta_HI, fvsn, luts, mc, trc, advanced_wfm_flags, eb, sb, reserved0_1, reserved0_2, reserved0_3, reserved0_4, reserved0_5, cs2] =
			self.__class__
				.parser
				.unpack(data[..self.__class__.structSize]);
		self.xwia = ((xwia_HI << 16) | xwia_LO);
		self.wmta = ((wmta_HI << 16) | wmta_LO);
	}
	fn __bytes__<RT>(&self) -> RT {
		let xwia_HI = (self.xwia >> 16);
		let xwia_LO = (self.xwia & 255);
		let wmta_HI = (self.wmta >> 16);
		let wmta_LO = (self.wmta & 255);
		return self.__class__.parser.pack((
			self.checksum,
			self.filesize,
			self.serial,
			self.run_type,
			self.fpl_platform,
			self.fpl_lot,
			self.mode_version_or_adhesive_run_num,
			self.waveform_version,
			self.waveform_subversion,
			self.waveform_type,
			self.fpl_size,
			self.mfg_code,
			self.waveform_tuning_bias_or_rev,
			self.fpl_rate,
			self.unknown0,
			self.vcom_shifted,
			self.unknown1,
			xwia_LO,
			xwia_HI,
			self.cs1,
			wmta_LO,
			wmta_HI,
			self.fvsn,
			self.luts,
			self.mc,
			self.trc,
			self.advanced_wfm_flags,
			self.eb,
			self.sb,
			self.reserved0_1,
			self.reserved0_2,
			self.reserved0_3,
			self.reserved0_4,
			self.reserved0_5,
			self.cs2,
		));
	}
}
struct pointer {
	addr: ST0,
}

/*
"
	struct pointer {
					u16 addr_LO;
					u8 addr_HI;
					u8 checksum;
	}__attribute__((packed));";
*/
impl pointer {
	//__slots__ = ("addr", "checksum");
	//structStr = "HBB";
	//parser = struct_.Struct(structStr);
	//structSize = struct_.calcsize(structStr);
	fn __init__(&self, data: &[u8]) {
		let [addr_LO, addr_HI, checksum] = self
			.__class__
			.parser
			.unpack(data[..self.__class__.structSize]);
		self.addr = ((addr_HI << 16) | addr_LO);
	}
}

/*
"
	struct temp_range {
			u8 from;
			u8 to;
	};";
*/
fn temp_range<RT>(data: &[u8]) -> RT {
	return (0..starred!(struct_.unpack("HBB", data))/*unsupported*/);
}
struct packed_state {
	s0: ST0,
	s1: ST1,
	s2: ST2,
	s3: ST3,
}

/*
"
	struct packed_state {
			u8 s0:2;
			u8 s1:2;
			u8 s2:2;
			u8 s3:2;
	}__attribute__((packed));";
*/

impl packed_state {
	//__slots__ = ("s0", "s1", "s2", "s3");
	fn __init__(&self, b: i32) {
		self.s0 = (b & 3);
		self.s1 = ((b >> 2) & 3);
		self.s2 = ((b >> 4) & 3);
		self.s3 = ((b >> 6) & 3);
	}
}
const unpacked_state: _ = packed_state;
/*"
struct unpacked_state {
	u8 s0;
	u8 s1;
	u8 s2;
	u8 s3;
}__attribute__((packed));
";
*/
fn get_bits_per_pixel(header: waveform_data_header) -> u8 {
	return u8(if (header.luts & 12) == 4 { 5 } else { 4 });
}
fn compare_checksum(data: &str, header: waveform_data_header) -> i32 {
	if crc32((b"\x00\x00\x00\x00" + data[4..header.filesize])) != header.checksum {
		return -1;
	}
	return 0;
}
fn add_addr(addrs: List<u32>, addr: u32, max: u32) -> i32 {
	let i: u32 = 0;
	for i in (0..max) {
		if addrs[i] == addr {
			return 0;
		}
		if !addrs[i] {
			addrs[i] = addr;
			return 1;
		}
	}
	println!("{:?} ", "Encountered more addresses than our hardcoded max");
	return -1;
}
fn pri32_header(header: waveform_data_header, is_wbf: i32) {
	println!("{:?} ", "Header info:");
	if is_wbf {
		println!(
			"{:?} ",
			(("\tFile size (according to header): " + String::from(header.filesize)) + " bytes")
		);
	}
	println!(
		"{:?} ",
		(("\tSerial number: " + String::from(header.serial)) + "")
	);
	println!(
		"{:?} ",
		(((("\tRun type: " + hex(header.run_type)) + " | ")
			+ get_desc(run_types, header.run_type, "Unknown"))
			+ "")
	);
	println!(
		"{:?} ",
		(((("\tManufacturer code: " + hex(header.mfg_code)) + " | ")
			+ get_desc_mfg_code(header.mfg_code))
			+ "")
	);
	println!(
		"{:?} ",
		(((("\tFrontplane Laminate (FPL) platform: " + hex(header.fpl_platform)) + " | ")
			+ get_desc(fpl_platforms, header.fpl_platform, "Unknown"))
			+ "")
	);
	println!(
		"{:?} ",
		(("\tFrontplane Laminate (FPL) lot: " + String::from(header.fpl_lot)) + "")
	);
	println!(
		"{:?} ",
		(((("\tFrontplane Laminate (FPL) size: " + hex(header.fpl_size)) + " | ")
			+ get_desc(fpl_sizes, header.fpl_size, "Unknown"))
			+ "")
	);
	println!(
		"{:?} ",
		(((("\tFrontplane Laminate (FPL) rate: " + hex(header.fpl_rate)) + " | ")
			+ get_desc(fpl_rates, header.fpl_rate, "Unknown"))
			+ "")
	);
	println!(
		"{:?} ",
		(("\tWaveform version: " + String::from(header.waveform_version)) + "")
	);
	println!(
		"{:?} ",
		(("\tWaveform sub-version: " + String::from(header.waveform_subversion)) + "")
	);
	println!(
		"{:?} ",
		(((("\tWaveform type: " + hex(header.waveform_type)) + " | ")
			+ get_desc(waveform_types, header.waveform_type, "Unknown"))
			+ "")
	);
	if header.waveform_type <= 21 {
		println!(
			"{:?} ",
			(((("\tWaveform tuning bias: " + hex(header.waveform_tuning_bias_or_rev)) + " | ")
				+ get_desc(
					waveform_tuning_biases,
					header.waveform_tuning_bias_or_rev,
					None
				))
				+ "")
		);
		println!("{:?} ", "\tWaveform revision: Unknown");
	} else {
		if header.waveform_type >= 43 {
			println!("{:?} ", "\tWaveform tuning bias: Unknown");
			println!(
				"{:?} ",
				(("\tWaveform revision: " + String::from(header.waveform_tuning_bias_or_rev)) + "")
			);
		} else {
			println!("{:?} ", "\tWaveform tuning bias: Unknown");
			println!("{:?} ", "\tWaveform revision: Unknown");
		}
	}
	if header.fpl_platform < 3 {
		println!(
			"{:?} ",
			(("\tAdhesive run number: " + String::from(header.mode_version_or_adhesive_run_num))
				+ "")
		);
		println!("{:?} ", "\tMode version: Unknown");
	} else {
		println!("{:?} ", "\tAdhesive run number: Unknown");
		println!(
			"{:?} ",
			(((("\tMode version: " + hex(header.mode_version_or_adhesive_run_num)) + " | ")
				+ get_desc(mode_versions, header.mode_version_or_adhesive_run_num, None))
				+ "")
		);
	}
	println!(
		"{:?} ",
		(("\tNumber of modes in this waveform: " + String::from((header.mc + 1))) + "")
	);
	println!(
		"{:?} ",
		(("\tNumber of temperature ranges in this waveform: " + String::from((header.trc + 1))) + "")
	);
	println!(
		"{:?} ",
		(("\t4 or 5-bits per pixel: " + String::from(get_bits_per_pixel(header))) + "")
	);
	println!("{:?} ", "");
}
fn get_waveform_length(wav_addrs: List<u32>, wav_addr: u32) -> u32 {
	let i: u32 = 0;
	for i in (0..(MAX_WAVEFORMS - 1)) {
		if wav_addrs[i] == wav_addr {
			if !wav_addrs[i] {
				return u32(0);
			}
			return u32((wav_addrs[(i + 1)] - wav_addr));
		}
	}
	return u32(0);
}
fn toS<RT>(n: i32) -> RT {
	if (n & 128) {
		return (n - 128);
	}
	return n;
}
fn parse_waveform_refactored(
	data: &[u8],
	wav_addrs: List<u32>,
	wav_addr: u32,
	outfile: io::IOBase,
) -> i32 {
	let i: u32 = 0;
	let j: u32 = 0;
	let mut s = None;
	let u = None;
	let mut count = None;
	let mut fc_active = 0;
	let mut zero_pad = 0;
	let written = None;
	let mut state_count = 0;
	let waveform = data[wav_addr..];
	let l = (get_waveform_length(wav_addrs, wav_addr) - 2);
	if !l {
		println!("{:?} ", "Could not find waveform length");
		return -1;
	}
	while i < (l - 1) {
		println!("{:?} ", "{}, {}".format(i, hex(waveform[i])[2..]));
		let is_terminator = waveform[i] == 252;
		if is_terminator {
			fc_active = !fc_active;
			i += 1;
		} else {
			s = packed_state(waveform[i]);
			if fc_active {
				count = 1;
				zero_pad = 1;
				i += 1;
			} else {
				if i >= (l - 1) {
					count = 1;
				} else {
					count = ((waveform[(i + 1)] & 255) + 1);
				}
				zero_pad = 0;
				i += 2;
			}
			println!("{:?} ", "count {:d}".format(count));
			state_count += ((count * 4) & 65535);
			if outfile {
				for j in (0..count) {
					outfile.write(struct_.pack("BBBB", s.s0, s.s1, s.s2, s.s3));
				}
			}
		}
	}
	return state_count;
}
fn parse_waveform(data: &[u8], wav_addrs: List<u32>, wav_addr: u32, outfile: io::IOBase) -> i32 {
	return parse_waveform_refactored(data, wav_addrs, wav_addr, outfile);
	let i: u32 = 0;
	let j: u32 = 0;
	let mut s = None;
	let u = None;
	let mut count = None;
	let mut fc_active = 0;
	let mut zero_pad = 0;
	let written = None;
	let mut state_count = 0;
	let waveform = data[wav_addr..];
	let l = (get_waveform_length(wav_addrs, wav_addr) - 2);
	if !l {
		println!("{:?} ", "Could not find waveform length");
		return -1;
	}
	while i < (l - 1) {
		println!("{:?} ", "{}, {}".format(i, hex(waveform[i])[2..]));
		ic(i, waveform[i]);
		if (waveform[i] & 255) == 252 {
			fc_active = if fc_active { 0 } else { 1 };
			i += 1;
			continue;
		}
		s = packed_state(waveform[i]);
		if fc_active {
			count = 1;
			zero_pad = 1;
			i += 1;
		} else {
			if i >= (l - 1) {
				count = 1;
			} else {
				count = ((waveform[(i + 1)] & 255) + 1);
			}
			zero_pad = 0;
			i += 2;
		}
		println!("{:?} ", "count {:d}".format(count));
		state_count += ((count * 4) & 65535);
		if outfile {
			for j in (0..count) {
				outfile.write(struct_.pack("BBBB", s.s0, s.s1, s.s2, s.s3));
			}
		}
	}
	return state_count;
}
fn parse_temp_ranges(
	header: waveform_data_header,
	data: &str,
	tr_start: &str,
	tr_count: u8,
	wav_addrs: List<u32>,
	first_pass: i32,
	outfile: io::IOBase,
	do_pri32: i32,
) -> i32 {
	let tr: pointer = None;
	let checksum: u8 = None;
	let i: u8 = None;
	let state_count: u16 = None;
	let written: size_t = None;
	let ftable: long = None;
	let fprev: long = None;
	let fcur: long = None;
	let tr_addrs = std::vector(256, 0);
	let tr_table_addr: u32 = None;
	if !tr_count {
		return 0;
	}
	if do_pri32 {
		println!("{:?} ", "\t\tTemperature ranges: ");
	}
	if outfile {
		let mut ftable = outfile.tell();
		outfile.seek(((header.trc + 1) * 8), SEEK_CUR);
	}
	for i in (0..tr_count) {
		if do_pri32 {
			sys.stdout.write("\t\t\tChecking range {:2d}: ".format(i));
		}
		let mut tr = pointer(tr_start);
		let mut checksum = (((tr_start[0] + tr_start[1]) + tr_start[2]) & 255);
		if checksum != tr.checksum {
			if do_pri32 {
				println!("{:?} ", "Failed");
			}
			return -1;
		}
		if first_pass {
			if add_addr(wav_addrs, tr.addr, MAX_WAVEFORMS) < 0 {
				return -1;
			}
		} else {
			if outfile {
				let mut fprev = outfile.tell();
				if add_addr(tr_addrs, (fprev - MYSTERIOUS_OFFSET), MAX_TEMP_RANGES) < 0 {
					return -1;
				}
				outfile.seek(8, SEEK_CUR);
			}
			let mut state_count = parse_waveform(data, wav_addrs, tr.addr, outfile);
			if state_count < 0 {
				return -1;
			}
			if do_pri32 {
				println!(
					"{:?} ",
					"{:4d} phases ({:4d})".format((state_count >> 8), tr.addr)
				);
			}
			if outfile {
				let mut fcur = outfile.tell();
				outfile.seek(fprev, SEEK_SET);
				let mut written = outfile.write(state_count.pack(">H"));
				outfile.fseek(fcur, SEEK_SET);
			}
		}
		tr_start = tr_start[4..];
	}
	if do_pri32 {
		println!("{:?} ", "");
	}
	if outfile {
		if write_table(ftable, tr_addrs, outfile, MAX_TEMP_RANGES) < 0 {
			println!("{:?} ", "Error writing temperature range table");
			return -1;
		}
	}
	return 0;
}
fn parse_modes(
	header: waveform_data_header,
	data: &str,
	mode_start: &str,
	mode_count: u8,
	temp_range_count: u8,
	wav_addrs: List<u32>,
	first_pass: i32,
	outfile: io::IOBase,
	do_pri32: i32,
) -> i32 {
	let mode: pointer = None;
	let checksum: u8 = None;
	let i: u8 = None;
	let pos: i32 = None;
	let mode_addrs: List<u32> = std::vector(256, 0);
	let mode_table_addr: u32 = None;
	if !mode_count {
		return 0;
	}
	if do_pri32 {
		println!("{:?} ", "Modes: ");
	}
	for i in (0..mode_count) {
		if do_pri32 {
			sys.stdout.write("\tChecking mode {:2d}: ".format(i));
		}
		let mut mode = pointer(mode_start);
		let mut checksum = (((mode_start[0] + mode_start[1]) + mode_start[2]) & 255);
		if checksum != mode.checksum {
			if do_pri32 {
				println!("{:?} ", "Failed");
			}
			return -1;
		}
		if outfile {
			let mut pos = (outfile.tell() - MYSTERIOUS_OFFSET);
			if add_addr(mode_addrs, pos, MAX_MODES) < 0 {
				return -1;
			}
		}
		if do_pri32 {
			println!("{:?} ", "Passed");
		}
		if parse_temp_ranges(
			header,
			data,
			data[mode.addr..],
			temp_range_count,
			wav_addrs,
			first_pass,
			outfile,
			do_pri32,
		) < 0
		{
			return -1;
		}
		mode_start = mode_start[4..];
	}
	if outfile {
		let mut mode_table_addr = ((waveform_data_header::structSize + header.trc) + 2);
		if write_table(mode_table_addr, mode_addrs, outfile, MAX_MODES) < 0 {
			println!("{:?} ", "Error writing mode table");
			return -1;
		}
	}
	return 0;
}
fn check_xwia(xwia: &str, do_pri32: i32) -> i32 {
	let xwia_len = xwia[0];
	let i: u8 = None;
	let checksum: u8 = xwia_len;
	let non_pri32ables: i32 = 0;
	xwia = xwia[1..((1 + xwia_len) + 1)];
	let xwia_s = xwia[..-1].decode("ascii");
	for i in (0..xwia_len) {
		if !xwia_s[i].ispri32able() {
			non_pri32ables += 1;
		}
		checksum += xwia[i];
	}
	if do_pri32 {
		sys.stdout
			.write("Extra Waveform Info (probably waveform's original filename): ");
		if !xwia_len {
			println!("{:?} ", "None");
		} else {
			if non_pri32ables {
				println!(
					"{:?} ",
					(((("(" + String::from(xwia_len)) + " bytes containing ")
						+ String::from(non_pri32ables))
						+ " unpri32able characters)")
				);
			} else {
				println!("{:?} ", xwia_s);
			}
		}
		println!("{:?} ", "");
	}
	if (checksum & 255) != xwia[xwia_len] {
		return -1;
	}
	return 0;
}
fn parse_temp_range_table(table: &str, range_count: u8, outfile: io::IOBase, do_pri32: i32) -> i32 {
	let i: u8 = None;
	let checksum: u8 = None;
	let written: size_t = None;
	if !range_count {
		return 0;
	}
	if do_pri32 {
		println!("{:?} ", "Supported temperature ranges:");
	}
	let mut checksum = 0;
	for i in (0..range_count) {
		let rng = (u8(table[i])..u8(table[(i + 1)]));
		if do_pri32 {
			println!(
				"{:?} ",
				(((("\t" + String::from(rng.start)) + " - ") + String::from(rng.stop)) + " °C")
			);
		}
		checksum = u8((checksum + rng.start));
	}
	checksum = u8((checksum + rng.stop));
	if (checksum & 255) != u8(table[(range_count + 1)]) {
		return -1;
	}
	if outfile {
		let mut written = outfile.fwrite(table, 1, (range_count + 1));
		if written != (range_count + 1) {
			println!(
				"{:?} ",
				(("Error writing temperature range table to output file: " + strerror(errno))
					+ "\n")
			);
			return -1;
		}
	}
	if do_pri32 {
		println!("{:?} ", "");
	}
	return 0;
}
fn write_table(table_addr: u32, addrs: List<u32>, outfile: io::IOBase, max: u32) -> i32 {
	let i: i32 = 0;
	let written: size_t = 0;
	let addr: u32 = 0;
	let prev: i32 = 0;
	let mut prev = outfile.tell();
	if prev < 0 {
		return -1;
	}
	outfile.seek(table_addr, SEEK_SET);
	for i in (0..max) {
		if !addrs[i] {
			break;
		}
		let mut addr = addrs[i];
		let mut written = outfile.fwrite(struct_.pack("I", addr));
		if written != struct_.calcsize("I") {
			println!(
				"{:?} ",
				(("Error writing address table to output file: " + strerror(errno)) + "\n")
			);
			return -1;
		}
		outfile.seek(4, SEEK_CUR);
	}
	outfile.seek(prev, SEEK_SET);
	return 0;
}
fn write_header(outfile: io::IOBase, header: waveform_data_header) -> i32 {
	let written: size_t = 0;
	let mut written = outfile.write(header.serialize());
	if written < waveform_data_header::structSize {
		return -1;
	}
	return 0;
}

fn mainAPI(infile_path: Path, force_input: bool, outfile_path: Option<Path>) -> i32 {
	let infile: io::IOBase = None;
	let header: waveform_data_header = None;
	let modes: str = None;
	let temp_range_table: str = None;
	let xwia_len: u32 = None;
	let mode_count: u8 = None;
	let temp_range_count: u8 = None;
	let outfile_path: Path = None;
	let outfile: io::IOBase = None;
	let do_pri32: i32 = 0;
	let force: i32 = 0;
	let c: i32 = None;
	let unique_waveform_count: u32 = None;
	let wav_addrs = (vec![0] * MAX_WAVEFORMS);
	let is_wbf: u32 = None;
	let to_alloc: size_t = None;
	if force_input {
		if force_input == "wbf" {
			let mut is_wbf = 1;
		} else {
			if force_input == "wrf" {
				is_wbf = 0;
			} else {
				println!("{:?} ", "Only wbf and wrf format is supported");
				raise!(Exception); //unsupported
			}
		}
	} else {
		if !infile_path.suffix {
			println!("{:?} ", "File has neither .wbf or .wrf extension");
			println!(
				"{:?} ",
				"Consider using `-f` to bypass file format detection"
			);
			raise!(Exception); //unsupported
		}
		if infile_path.suffix == ".wbf" {
			let mut is_wbf = 1;
		} else {
			if infile_path.suffix == ".wrf" {
				is_wbf = 0;
			} else {
				println!("{:?} ", "File has neither .wbf or .wrf extension");
				println!(
					"{:?} ",
					"Consider using `-f` to bypass file format detection"
				);
				raise!(Exception); //unsupported
			}
		}
	}
	if !is_wbf && outfile_path {
		println!("{:?} ", "Conversion from .wrf format not supported");
		raise!(Exception); //unsupported
	}
	// with!(infile_path.open("rb") as infile) //unsupported
	{
		let st = infile_path.stat();
		if is_wbf {
			to_alloc = st.st_size;
		} else {
			to_alloc = waveform_data_header::structSize;
		}
		// with!(mmap.mmap(infile.fileno(), to_alloc, mmap.ACCESS_READ) as data) //unsupported
		{
			if outfile_path {
				let mut outfile = outfile_path.open("wb");
			}
			if !outfile {
				let mut do_pri32 = 1;
			}
			if do_pri32 {
				println!("{:?} ", "");
				println!(
					"{:?} ",
					(("File size: " + String::from(st.st_size)) + " bytes")
				);
				println!("{:?} ", "");
			}
			let mut header = waveform_data_header(data);
			if is_wbf {
				if header.filesize != st.st_size {
					println!(
						"{:?} ",
						"Actual file size does not match file size reported by waveform header"
					);
					raise!(Exception); //unsupported
				}
			}
			if outfile {
				if get_bits_per_pixel(header) != 4 {
					println!(
						"{:?} ",
						"This waveform uses 5 bits per pixel which is not yet support"
					);
					raise!(Exception); //unsupported
				}
			}
			if is_wbf {
				if compare_checksum(data, header) < 0 {
					println!("{:?} ", "Checksum error");
					raise!(Exception); //unsupported
				}
			}
			if do_pri32 {
				pri32_header(header, is_wbf);
				if header.fpl_platform < 3 {
					println!("{:?} ", "Modes: Unknown (no mode version specified)");
				} else {
					pri32_modes((header.mc + 1));
				}
			}
			if !is_wbf {
				return 0;
			}
			if outfile {
				if write_header(outfile, header) < 0 {
					println!("{:?} ", "Writing header to output failed");
					raise!(Exception); //unsupported
				}
			}
			let mut temp_range_table = data[waveform_data_header::structSize..];
			if parse_temp_range_table(temp_range_table, (header.trc + 1), outfile, do_pri32) {
				println!("{:?} ", "Temperature range checksum error");
				raise!(Exception); //unsupported
			}
			if outfile {
				outfile.seek((8 * (header.mc + 1)), SEEK_CUR);
			}
			if header.xwia {
				xwia_len = data[header.xwia];
				if check_xwia(data[header.xwia..], do_pri32) < 0 {
					println!("{:?} ", "xwia checksum error");
					raise!(Exception); //unsupported
				}
			} else {
				xwia_len = 0;
			}
			let mut modes = data[(((header.xwia + 1) + xwia_len) + 1)..];
			if parse_modes(
				header,
				data,
				modes,
				(header.mc + 1),
				(header.trc + 1),
				wav_addrs,
				1,
				None,
				0,
			) < 0
			{
				println!("{:?} ", "Parse error during first pass");
				raise!(Exception); //unsupported
			}
			let mut unique_waveform_count = 0;
			for wfa in wav_addrs {
				if !wfa {
					break;
				}
				unique_waveform_count += 1;
			}
			let wav_addrs_uniq = wav_addrs[..unique_waveform_count];
			wav_addrs_uniq.sort();
			wav_addrs_uniq.drop();
			if do_pri32 {
				println!(
					"{:?} ",
					(("Number of unique waveforms: " + String::from(unique_waveform_count)) + "\n")
				);
			}
			if add_addr(wav_addrs, st.st_size, MAX_WAVEFORMS) < 0 {
				println!("{:?} ", "Failed to add file end address to waveform table.");
				raise!(Exception); //unsupported
			}
			if parse_modes(
				header,
				data,
				modes,
				(header.mc + 1),
				(header.trc + 1),
				wav_addrs,
				0,
				outfile,
				do_pri32,
			) < 0
			{
				println!("{:?} ", "Parse error during second pass");
				raise!(Exception); //unsupported
			}
			return 0;
		}
	}
	return 1;
}
//"Convert a .wbf file to a .wrf file or if no output file is specified display human readable info about the specified .wbf or .wrf file.";
//const USAGE: _ = "inkwave file.wbf/file.wrf [-o output.wrf]";
//const outfile_path: _ = cli::SwitchAttr("-o", "Specify output file");
//const force_input: _ = cli::SwitchAttr("-f", "Force inkwave to i32erpret input file as either .wrf or .wbf format regardless of file extension");
fn main() {
	return mainAPI(
		Path(infile_path),
		force_input,
		if outfile_path {
			Path(outfile_path)
		} else {
			None
		},
	);
}
