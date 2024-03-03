use crate::opcodes;
use std::collections::HashMap;

/// # Status Register (P) http://wiki.nesdev.com/w/index.php/Status_flags
///
///  7 6 5 4 3 2 1 0
///  N V _ B D I Z C
///  | |   | | | | +--- Carry Flag
///  | |   | | | +----- Zero Flag
///  | |   | | +------- Interrupt Disable
///  | |   | +--------- Decimal Mode (not used on NES)
///  | |   +----------- Break Command
///  | +--------------- OFLAG_ZEROverflow Flag
///  +----------------- Negative Flag
///
const STATUS_CARRY: u8 = 0b0000_0001;
const STATUS_ZERO: u8 = 0b0000_0010;
const STATUS_INTERRUPT_DISABLE: u8 = 0b0000_0100;
const STATUS_DECIMAL_MODE: u8 = 0b0000_1000;
const STATUS_BREAK: u8 = 0b0001_0000;
const STATUS_BREAK2: u8 = 0b0010_0000;
const STATUS_OVERFLOW: u8 = 0b0100_0000;
const STATUS_NEGATIVE: u8 = 0b1000_0000;

#[derive(Debug)]
#[allow(non_camel_case_types)]
pub enum AddressingMode {
    Immediate,
    ZeroPage,
    ZeroPage_X,
    ZeroPage_Y,
    Absolute,
    Absolute_X,
    Absolute_Y,
    Indirect_X,
    Indirect_Y,
    NoneAddressing,
    Relative,
}

trait Mem {
    fn mem_read(&self, addr: u16) -> u8;

    fn mem_write(&mut self, addr: u16, data: u8);

    fn mem_read_u16(&self, pos: u16) -> u16 {
        let lo = self.mem_read(pos) as u16;
        let hi = self.mem_read(pos + 1) as u16;
        (hi << 8) | (lo as u16)
    }

    fn mem_write_u16(&mut self, pos: u16, data: u16) {
        let hi = (data >> 8) as u8;
        let lo = (data & 0xff) as u8;
        self.mem_write(pos, lo);
        self.mem_write(pos + 1, hi);
    }
}

impl Mem for CPU {
    fn mem_read(&self, addr: u16) -> u8 {
        self.memory[addr as usize]
    }

    fn mem_write(&mut self, addr: u16, data: u8) {
        self.memory[addr as usize] = data;
    }
}

pub struct CPU {
    pub register_a: u8,
    pub register_x: u8,
    pub register_y: u8,
    pub status: u8,
    pub program_counter: u16,
    memory: [u8; 0x10000],
}

impl CPU {
    pub fn new() -> Self {
        CPU {
            register_a: 0,
            register_x: 0,
            register_y: 0,
            status: 0,
            program_counter: 0,
            memory: [0; 0x10000],
        }
    }

    fn get_operand_address(&self, mode: &AddressingMode) -> u16 {
        match mode {
            AddressingMode::Immediate => self.program_counter,

            AddressingMode::ZeroPage => self.mem_read(self.program_counter) as u16,

            AddressingMode::Absolute => self.mem_read_u16(self.program_counter),

            AddressingMode::ZeroPage_X => {
                let pos = self.mem_read(self.program_counter);
                let addr = pos.wrapping_add(self.register_x) as u16;
                addr
            }
            AddressingMode::ZeroPage_Y => {
                let pos = self.mem_read(self.program_counter);
                let addr = pos.wrapping_add(self.register_y) as u16;
                addr
            }

            AddressingMode::Absolute_X => {
                let base = self.mem_read_u16(self.program_counter);
                let addr = base.wrapping_add(self.register_x as u16);
                addr
            }
            AddressingMode::Absolute_Y => {
                let base = self.mem_read_u16(self.program_counter);
                let addr = base.wrapping_add(self.register_y as u16);
                addr
            }

            AddressingMode::Indirect_X => {
                let base: u8 = self.mem_read(self.program_counter);

                let ptr: u8 = (base as u8).wrapping_add(self.register_x);
                let lo = self.mem_read(ptr as u16);
                let hi = self.mem_read(ptr.wrapping_add(1) as u16);
                (hi as u16) << 8 | (lo as u16)
            }
            AddressingMode::Indirect_Y => {
                let base = self.mem_read(self.program_counter);

                let lo = self.mem_read(base as u16);
                let hi = self.mem_read((base as u8).wrapping_add(1) as u16);
                let deref_base = (hi as u16) << 8 | (lo as u16);
                let deref = deref_base.wrapping_add(self.register_y as u16);
                deref
            }

            AddressingMode::NoneAddressing => {
                panic!("mode {:?} is not supported", mode);
            }

            AddressingMode::Relative => {
                panic!("mode {:?} is not supported", mode);
            }
        }
    }

    fn mem_read(&self, addr: u16) -> u8 {
        self.memory[addr as usize]
    }

    fn mem_write(&mut self, addr: u16, data: u8) {
        self.memory[addr as usize] = data;
    }

    fn mem_read_u16(&self, pos: u16) -> u16 {
        let lo = self.mem_read(pos) as u16;
        let hi = self.mem_read(pos + 1) as u16;
        (hi << 8) | (lo as u16)
    }

    fn mem_write_u16(&mut self, pos: u16, data: u16) {
        let hi = (data >> 8) as u8;
        let lo = (data & 0xff) as u8;
        self.mem_write(pos, lo);
        self.mem_write(pos + 1, hi);
    }

    pub fn load(&mut self, program: Vec<u8>) {
        self.memory[0x8000..(0x8000 + program.len())].copy_from_slice(&program[..]);
        self.mem_write_u16(0xFFFC, 0x8000);
    }

    pub fn load_and_run(&mut self, program: Vec<u8>) {
        self.load(program);
        self.reset();
        self.run()
    }

    pub fn reset(&mut self) {
        println!("Resetting CPU");
        self.register_a = 0;
        self.register_x = 0;
        self.status = 0;

        self.program_counter = self.mem_read_u16(0xFFFC);
    }

    fn lda(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);

        self.register_a = value;
        self.update_zero_and_negative_flags(self.register_a);
    }

    fn sta(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.register_a;
        self.mem_write(addr, value);
    }

    fn adc(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);
        let carry_flag = self.status & STATUS_CARRY;

        let (rhs, overflow) = value.overflowing_add(carry_flag);
        let (result, overflow2) = self.register_a.overflowing_add(rhs);

        if overflow || overflow2 {
            self.status = self.status | STATUS_CARRY;
        } else {
            self.status = self.status & !STATUS_CARRY;
        }

        if (result ^ value) & (result ^ self.register_a) & STATUS_NEGATIVE != 0 {
            self.status = self.status | STATUS_OVERFLOW;
        } else {
            self.status = self.status & !STATUS_OVERFLOW;
        }

        self.register_a = result;
        self.update_zero_and_negative_flags(self.register_a);
    }

    fn tax(&mut self) {
        self.register_x = self.register_a;
        self.update_zero_and_negative_flags(self.register_x);
    }

    fn inx(&mut self) {
        println!("inx");
        self.register_x = self.register_x.wrapping_add(1);
        self.update_zero_and_negative_flags(self.register_x);
    }

    fn update_zero_and_negative_flags(&mut self, result: u8) {
        if result == 0 {
            self.status = self.status | STATUS_ZERO;
        } else {
            self.status = self.status & !STATUS_ZERO;
        }

        if result & STATUS_NEGATIVE != 0 {
            self.status = self.status | STATUS_NEGATIVE;
        } else {
            self.status = self.status & 0b0111_1111;
        }
    }

    pub fn run(&mut self) {
        let ref opcodes: HashMap<u8, &'static opcodes::OpCode> = *opcodes::OPCODES_MAP;

        loop {
            let code = self.mem_read(self.program_counter);
            println!(
                "run opscode: {:x}, program_counter: {:x}",
                code, self.program_counter
            );
            self.program_counter += 1;

            let program_counter_state = self.program_counter;
            let opcode = opcodes
                .get(&code)
                .expect(&format!("OpCode {:x} is not recognized", code));

            match code {
                0xa9 | 0xa5 | 0xb5 | 0xad | 0xbd | 0xb9 | 0xa1 | 0xb1 => {
                    self.lda(&opcode.mode);
                }
                /* STA */
                0x85 | 0x95 | 0x8d | 0x9d | 0x99 | 0x81 | 0x91 => {
                    self.sta(&opcode.mode);
                }
                /* ADC */
                0x69 | 0x65 | 0x75 | 0x6d | 0x7d | 0x79 | 0x61 | 0x71 => {
                    self.adc(&opcode.mode);
                }
                0xAA => self.tax(),
                0xE8 => self.inx(),
                // 0x10 => self.bpl(&opcode.mode),
                0x00 => return,
                _ => todo!(),
            }

            if program_counter_state == self.program_counter {
                self.program_counter += (opcode.len - 1) as u16;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_0xa9_lda_immediate_load_data() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xa9, 0x05, 0x00]);
        assert_eq!(cpu.register_a, 0x05);
        assert!(cpu.status & STATUS_ZERO == 0b00);
        assert!(cpu.status & STATUS_NEGATIVE == 0b0000_0000);
    }

    #[test]
    fn test_0xa9_lda_zero_flag() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xa9, 0x00, 0x00]);
        assert!(cpu.status & STATUS_ZERO == 0b10);
        assert!(cpu.status & STATUS_NEGATIVE == 0b0000_0000);
    }

    #[test]
    fn test_0xa9_lda_negative_flag() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xa9, 0x80, 0x00]);
        assert!(cpu.status & STATUS_NEGATIVE == STATUS_NEGATIVE);
    }

    #[test]
    fn test_0xaa_tax_move_a_to_x() {
        let mut cpu = CPU::new();

        cpu.load(vec![0xaa, 0x00]);
        cpu.reset();
        cpu.register_a = 10;
        cpu.run();

        assert_eq!(cpu.register_x, 10);
        assert!(cpu.status & STATUS_ZERO == 0b00);
        assert!(cpu.status & STATUS_NEGATIVE == 0b0000_0000);
    }

    #[test]
    fn test_0xaa_tax_move_a_to_x_negative() {
        let mut cpu = CPU::new();

        cpu.load(vec![0xaa, 0x00]);
        cpu.reset();
        cpu.register_a = 0x80;
        cpu.run();

        assert_eq!(cpu.register_x, 0x80);
        assert!(cpu.status & STATUS_ZERO == 0b00);
        assert!(cpu.status & STATUS_NEGATIVE == STATUS_NEGATIVE);
    }

    #[test]
    fn test_5_ops_working_together() {
        let mut cpu = CPU::new();
        cpu.load(vec![0xa9, 0xc0, 0xaa, 0xe8, 0x00]);
        cpu.reset();
        cpu.register_x = 0xff;
        cpu.run();

        assert_eq!(cpu.register_x, 0xc1)
    }

    #[test]
    fn test_inx_overflow() {
        let mut cpu = CPU::new();
        cpu.load(vec![0xe8, 0xe8, 0x00]);
        cpu.reset();
        cpu.register_x = 0xff;
        cpu.run();

        assert_eq!(cpu.register_x, 1)
    }

    #[test]
    fn test_lda_from_memory_zero_page() {
        let mut cpu = CPU::new();
        cpu.load(vec![0xa5, 0x10, 0x00]);
        cpu.reset();
        cpu.mem_write(0x10, 0x55);
        cpu.run();

        assert_eq!(cpu.register_a, 0x55);
    }

    #[test]
    fn test_lda_from_memory_zero_page_x() {
        let mut cpu = CPU::new();
        cpu.load(vec![0xb5, 0x10, 0x00]);
        cpu.reset();
        cpu.register_x = 0x01;
        cpu.mem_write(0x11, 0x56);
        cpu.run();

        assert_eq!(cpu.register_a, 0x56);
    }

    #[test]
    fn test_lda_from_memory_absolute() {
        let mut cpu = CPU::new();
        cpu.load(vec![0xad, 0x10, 0x20, 0x00]);
        cpu.reset();
        cpu.mem_write(0x2010, 0x57);
        cpu.run();

        assert_eq!(cpu.register_a, 0x57);
    }

    #[test]
    fn test_lda_from_memory_absolute_x() {
        let mut cpu = CPU::new();
        cpu.load(vec![0xbd, 0x11, 0x21, 0x00]);
        cpu.reset();
        cpu.register_x = 0x01;
        cpu.mem_write(0x2112, 0x58);
        cpu.run();

        assert_eq!(cpu.register_a, 0x58);
    }

    #[test]
    fn test_lda_from_memory_absolute_y() {
        let mut cpu = CPU::new();
        cpu.load(vec![0xb9, 0x12, 0x22, 0x00]);
        cpu.reset();
        cpu.register_y = 0x02;
        cpu.mem_write(0x2214, 0x59);
        cpu.run();

        assert_eq!(cpu.register_a, 0x59);
    }

    #[test]
    fn test_lda_from_memory_indirect_x() {
        let mut cpu = CPU::new();
        cpu.load(vec![0xa1, 0x11, 0x00]);
        cpu.reset();
        cpu.register_x = 0x01;
        cpu.mem_write_u16(0x12, 0x3344);
        cpu.mem_write(0x3344, 0x60);
        cpu.run();

        assert_eq!(cpu.register_a, 0x60);
    }

    #[test]
    fn test_lda_from_memory_indirect_y() {
        let mut cpu = CPU::new();
        cpu.load(vec![0xb1, 0x12, 0x00]);
        cpu.reset();
        cpu.mem_write_u16(0x12, 0x3345);
        cpu.register_y = 0x02;
        cpu.mem_write(0x3347, 0x61);
        cpu.run();

        assert_eq!(cpu.register_a, 0x61);
    }

    #[test]
    fn test_sta_from_memory_zero_page() {
        let mut cpu = CPU::new();
        cpu.load(vec![0x85, 0x10, 0x00]);
        cpu.reset();
        cpu.register_a = 0x50;
        cpu.run();

        assert_eq!(cpu.mem_read(0x10), 0x50);
    }

    #[test]
    fn test_sta_from_memory_zero_page_x() {
        let mut cpu = CPU::new();
        cpu.load(vec![0x95, 0x10, 0x00]);
        cpu.reset();
        cpu.register_x = 0x01;
        cpu.register_a = 0x51;
        cpu.run();

        assert_eq!(cpu.mem_read(0x11), 0x51);
    }

    #[test]
    fn test_sta_from_memory_absolute() {
        let mut cpu = CPU::new();
        cpu.load(vec![0x8d, 0x20, 0x30, 0x00]);
        cpu.reset();
        cpu.register_a = 0x52;
        cpu.run();

        assert_eq!(cpu.mem_read(0x3020), 0x52);
    }

    #[test]
    fn test_sta_from_memory_absolute_x() {
        let mut cpu = CPU::new();
        cpu.load(vec![0x9d, 0x21, 0x31, 0x00]);
        cpu.reset();
        cpu.register_a = 0x53;
        cpu.register_x = 0x01;
        cpu.run();

        assert_eq!(cpu.mem_read(0x3122), 0x53);
    }

    #[test]
    fn test_sta_from_memory_absolute_y() {
        let mut cpu = CPU::new();
        cpu.load(vec![0x99, 0x22, 0x32, 0x00]);
        cpu.reset();
        cpu.register_a = 0x54;
        cpu.register_y = 0x02;
        cpu.run();

        assert_eq!(cpu.mem_read(0x3224), 0x54);
    }

    #[test]
    fn test_sta_from_memory_indirect_x() {
        let mut cpu = CPU::new();
        cpu.load(vec![0x81, 0x23, 0x00]);
        cpu.reset();
        cpu.register_x = 0x03;
        cpu.register_a = 0x55;
        cpu.mem_write_u16(0x26, 0x4455);
        cpu.run();

        assert_eq!(cpu.mem_read(0x4455), 0x55);
    }

    #[test]
    fn test_sta_from_memory_indirect_y() {
        let mut cpu = CPU::new();
        cpu.load(vec![0x91, 0x24, 0x00]);
        cpu.reset();
        cpu.mem_write_u16(0x24, 0x5566);
        cpu.register_y = 0x04;
        cpu.register_a = 0x56;
        cpu.run();

        assert_eq!(cpu.mem_read(0x556a), 0x56);
    }

    // ADC
    #[test]
    fn test_adc_no_carry() {
        let mut cpu = CPU::new();
        cpu.load(vec![0x69, 0x10, 0x00]);
        cpu.reset();
        cpu.register_a = 0x20;
        cpu.run();
        assert_eq!(cpu.register_a, 0x30);
        assert_eq!(cpu.status, 0)
    }

    #[test]
    fn test_adc_has_carry() {
        let mut cpu = CPU::new();
        cpu.load(vec![0x69, 0x10, 0x00]);
        cpu.reset();
        cpu.register_a = 0x20;
        cpu.status = STATUS_CARRY;
        cpu.run();
        assert_eq!(cpu.register_a, 0x31);
        assert_eq!(cpu.status, 0);
    }

    #[test]
    fn test_adc_occur_carry() {
        let mut cpu = CPU::new();
        cpu.load(vec![0x69, 0x01, 0x00]);
        cpu.reset();
        cpu.register_a = 0xFF;
        cpu.run();
        assert_eq!(cpu.register_a, 0x00);
        assert_eq!(cpu.status, STATUS_CARRY | STATUS_ZERO);
    }

    #[test]
    fn test_adc_occur_overflow_plus() {
        let mut cpu = CPU::new();
        cpu.load(vec![0x69, 0x10, 0x00]);
        cpu.reset();
        cpu.register_a = 0x7F;
        cpu.run();
        assert_eq!(cpu.register_a, 0x8F);
        assert_eq!(cpu.status, STATUS_NEGATIVE | STATUS_OVERFLOW);
    }

    #[test]
    fn test_adc_occur_overflow_plus_with_carry() {
        let mut cpu = CPU::new();
        cpu.load(vec![0x69, 0x6F, 0x00]);
        cpu.reset();
        cpu.register_a = 0x10;
        cpu.status = STATUS_CARRY;
        cpu.run();
        assert_eq!(cpu.register_a, 0x80);
        assert_eq!(cpu.status, STATUS_NEGATIVE | STATUS_OVERFLOW);
    }

    #[test]
    fn test_adc_occur_overflow_minus() {
        let mut cpu = CPU::new();
        cpu.load(vec![0x69, 0x81, 0x00]);
        cpu.reset();
        cpu.register_a = 0x81;
        cpu.run();
        assert_eq!(cpu.register_a, 0x02);
        assert_eq!(cpu.status, STATUS_OVERFLOW | STATUS_CARRY);
    }

    #[test]
    fn test_adc_occur_overflow_minus_with_carry() {
        let mut cpu = CPU::new();
        cpu.load(vec![0x69, 0x80, 0x00]);
        cpu.reset();
        cpu.register_a = 0x80;
        cpu.status = STATUS_CARRY;
        cpu.run();
        assert_eq!(cpu.register_a, 0x01);
        assert_eq!(cpu.status, STATUS_OVERFLOW | STATUS_CARRY);
    }

    #[test]
    fn test_adc_no_overflow() {
        let mut cpu = CPU::new();
        cpu.load(vec![0x69, 0x7F, 0x00]);
        cpu.reset();
        cpu.register_a = 0x82;
        cpu.run();
        assert_eq!(cpu.register_a, 0x01);
        assert_eq!(cpu.status, STATUS_CARRY);
    }
}
